package app

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"
)

type orderInput struct {
	Customer string `json:"customer"`
	Amount   int    `json:"amount"`
}

type orderResponse struct {
	ID       int    `json:"id"`
	Customer string `json:"customer"`
	Amount   int    `json:"amount"`
}

var Handler http.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
	switch r.URL.Path {
	case "/orders/health":
		w.Header().Set("Content-Type", "application/json")
		if err := databaseHealth(); err != nil {
			w.WriteHeader(http.StatusServiceUnavailable)
			_, _ = io.WriteString(w, `{"service":"orders","status":"database-unavailable"}`)
			return
		}
		_, _ = io.WriteString(w, `{"service":"orders","language":"go-net-http","release":"blue","status":"ready","database":"postgres"}`)
	case "/orders":
		if r.Method == http.MethodPost {
			var input orderInput
			if err := json.NewDecoder(r.Body).Decode(&input); err != nil || input.Customer == "" || input.Amount <= 0 {
				http.Error(w, "customer and positive amount are required", http.StatusBadRequest)
				return
			}
			id, err := createOrder(input.Customer, input.Amount)
			if err != nil {
				http.Error(w, "order persistence failed", http.StatusServiceUnavailable)
				return
			}
			writeJSON(w, http.StatusCreated, orderResponse{ID: id, Customer: input.Customer, Amount: input.Amount})
			return
		}
		if r.Method == http.MethodGet {
			orders, err := listOrders()
			if err != nil {
				http.Error(w, "order query failed", http.StatusServiceUnavailable)
				return
			}
			writeJSON(w, http.StatusOK, orders)
			return
		}
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	case "/orders/db":
		value, err := databaseHealthValue()
		if err != nil {
			http.Error(w, "database query failed", http.StatusServiceUnavailable)
			return
		}
		writeJSON(w, http.StatusOK, map[string]string{"service": "orders", "database": "postgres", "result": strconv.Itoa(value)})
	case "/orders/slow":
		time.Sleep(250 * time.Millisecond)
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"service":"orders","release":"blue","status":"slow-complete"}`)
	case "/orders/language":
		_, _ = io.WriteString(w, "go-net-http")
	case "/orders/echo":
		_, _ = io.Copy(w, r.Body)
	case "/orders/cpu":
		work := r.URL.Query().Get("work")
		iterations := uint64(250_000)
		if work == "large" {
			iterations = 1_500_000
		} else if work == "small" {
			iterations = 80_000
		}
		var total uint64
		for i := uint64(0); i < iterations; i++ {
			total += i * 31
		}
		_, _ = fmt.Fprintf(w, `{"service":"orders","work":%q,"checksum":%d}`, work, total)
	default:
		http.NotFound(w, r)
	}
})

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
