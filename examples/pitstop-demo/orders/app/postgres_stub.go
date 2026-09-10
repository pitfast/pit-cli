//go:build !wasm

package app

import "errors"

var errWasiPostgres = errors.New("PostgreSQL is available only in the PitFast WASI build")

func databaseHealth() error { return errWasiPostgres }

func databaseHealthValue() (int, error) { return 0, errWasiPostgres }

func createOrder(string, int) (int, error) { return 0, errWasiPostgres }

func listOrders() ([]orderResponse, error) { return nil, errWasiPostgres }
