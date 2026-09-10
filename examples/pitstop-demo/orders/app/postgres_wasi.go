//go:build wasm

package app

import (
	"context"
	"fmt"
	"io"
	"net"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/jackc/pgx/v5"
	witTypes "go.bytecodealliance.org/pkg/wit/types"
	"wit_component/wasi_io_streams"
	"wit_component/wasi_sockets_instance_network"
	"wit_component/wasi_sockets_network"
	"wit_component/wasi_sockets_tcp"
	"wit_component/wasi_sockets_tcp_create_socket"
)

type wasiPostgresConn struct {
	input  *wasi_io_streams.InputStream
	output *wasi_io_streams.OutputStream
	closed bool
}

func (c *wasiPostgresConn) Read(p []byte) (int, error) {
	if c.closed {
		return 0, net.ErrClosed
	}
	result := c.input.BlockingRead(uint64(len(p)))
	if result.IsErr() {
		return 0, fmt.Errorf("postgres input stream: %v", result.Err().Tag())
	}
	value := result.Ok()
	if len(value) == 0 {
		return 0, io.EOF
	}
	return copy(p, value), nil
}

func (c *wasiPostgresConn) Write(p []byte) (int, error) {
	if c.closed {
		return 0, net.ErrClosed
	}
	result := c.output.BlockingWriteAndFlush(p)
	if result.IsErr() {
		return 0, fmt.Errorf("postgres output stream: %v", result.Err().Tag())
	}
	return len(p), nil
}

func (c *wasiPostgresConn) Close() error {
	if !c.closed {
		c.closed = true
		c.input.Drop()
		c.output.Drop()
	}
	return nil
}

func (c *wasiPostgresConn) LocalAddr() net.Addr              { return &net.TCPAddr{} }
func (c *wasiPostgresConn) RemoteAddr() net.Addr             { return &net.TCPAddr{} }
func (c *wasiPostgresConn) SetDeadline(time.Time) error      { return nil }
func (c *wasiPostgresConn) SetReadDeadline(time.Time) error  { return nil }
func (c *wasiPostgresConn) SetWriteDeadline(time.Time) error { return nil }

func openDatabase() (*pgx.Conn, context.Context, error) {
	value := os.Getenv("DATABASE_URL")
	if value == "" {
		return nil, nil, fmt.Errorf("DATABASE_URL was not injected")
	}
	config, err := pgx.ParseConfig(value)
	if err != nil {
		return nil, nil, fmt.Errorf("parse DATABASE_URL: %w", err)
	}
	config.DialFunc = dialWasiTCP
	config.ConnectTimeout = 4 * time.Second
	ctx, cancel := context.WithTimeout(context.Background(), 4*time.Second)
	conn, err := pgx.ConnectConfig(ctx, config)
	if err != nil {
		cancel()
		return nil, nil, err
	}
	return conn, ctx, nil
}

func closeDatabase(conn *pgx.Conn, ctx context.Context) {
	_ = conn.Close(ctx)
}

func databaseHealth() error {
	value, err := databaseHealthValue()
	if err != nil {
		return err
	}
	if value != 1 {
		return fmt.Errorf("unexpected database result %d", value)
	}
	return nil
}

func databaseHealthValue() (int, error) {
	conn, ctx, err := openDatabase()
	if err != nil {
		return 0, err
	}
	defer closeDatabase(conn, ctx)
	var value int
	err = conn.QueryRow(ctx, "SELECT 1").Scan(&value)
	return value, err
}

func createOrder(customer string, amount int) (int, error) {
	conn, ctx, err := openDatabase()
	if err != nil {
		return 0, err
	}
	defer closeDatabase(conn, ctx)
	if _, err = conn.Exec(ctx, `CREATE TABLE IF NOT EXISTS demo_orders (
        id BIGSERIAL PRIMARY KEY,
        customer TEXT NOT NULL,
        amount INTEGER NOT NULL CHECK (amount > 0),
        created_at TIMESTAMPTZ NOT NULL DEFAULT now()
    )`); err != nil {
		return 0, err
	}
	var id int
	err = conn.QueryRow(ctx, "INSERT INTO demo_orders (customer, amount) VALUES ($1, $2) RETURNING id", customer, amount).Scan(&id)
	return id, err
}

func listOrders() ([]orderResponse, error) {
	conn, ctx, err := openDatabase()
	if err != nil {
		return nil, err
	}
	defer closeDatabase(conn, ctx)
	rows, err := conn.Query(ctx, "SELECT id, customer, amount FROM demo_orders ORDER BY id DESC LIMIT 25")
	if err != nil {
		if strings.Contains(err.Error(), "does not exist") {
			return []orderResponse{}, nil
		}
		return nil, err
	}
	defer rows.Close()
	orders := make([]orderResponse, 0, 25)
	for rows.Next() {
		var order orderResponse
		if err := rows.Scan(&order.ID, &order.Customer, &order.Amount); err != nil {
			return nil, err
		}
		orders = append(orders, order)
	}
	return orders, rows.Err()
}

func dialWasiTCP(ctx context.Context, network, address string) (net.Conn, error) {
	if network != "tcp" && network != "tcp4" {
		return nil, fmt.Errorf("unsupported PostgreSQL network %q", network)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	host, portText, err := net.SplitHostPort(address)
	if err != nil || host != "127.0.0.1" {
		return nil, fmt.Errorf("PostgreSQL gateway must be 127.0.0.1, got %q", address)
	}
	port, err := strconv.ParseUint(portText, 10, 16)
	if err != nil {
		return nil, fmt.Errorf("invalid PostgreSQL port: %w", err)
	}
	networkResource := wasi_sockets_instance_network.InstanceNetwork()
	defer networkResource.Drop()
	socketResult := wasi_sockets_tcp_create_socket.CreateTcpSocket(wasi_sockets_network.IpAddressFamilyIpv4)
	if socketResult.IsErr() {
		return nil, fmt.Errorf("create PostgreSQL socket: %d", socketResult.Err())
	}
	socket := socketResult.Ok()
	addressValue := wasi_sockets_network.MakeIpSocketAddressIpv4(wasi_sockets_network.Ipv4SocketAddress{
		Port: uint16(port), Address: witTypes.Tuple4[uint8, uint8, uint8, uint8]{F0: 127, F1: 0, F2: 0, F3: 1},
	})
	if result := socket.StartConnect(networkResource, addressValue); result.IsErr() {
		socket.Drop()
		return nil, fmt.Errorf("connect PostgreSQL gateway: %d", result.Err())
	}
	for {
		result := socket.FinishConnect()
		if result.IsOk() {
			streams := result.Ok()
			return &wasiPostgresConn{input: streams.F0, output: streams.F1}, nil
		}
		if result.Err() != wasi_sockets_network.ErrorCodeWouldBlock {
			socket.Drop()
			return nil, fmt.Errorf("finish PostgreSQL gateway connect: %d", result.Err())
		}
		socket.Subscribe().Block()
	}
}

// Keep the generated TCP binding referenced even when pgx's dial path is
// optimized by the compiler; this ensures componentize-go retains the WIT
// socket imports needed by the host policy.
var _ *wasi_sockets_tcp.TcpSocket
