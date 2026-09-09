package app

import (
	"fmt"
	"io"
	"net/http"
	"time"
)

var Handler http.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
	switch r.URL.Path {
	case "/orders/health":
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"service":"orders","language":"go-net-http","release":"blue","status":"ready"}`)
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
		if work == "large" { iterations = 1_500_000 } else if work == "small" { iterations = 80_000 }
		var total uint64
		for i := uint64(0); i < iterations; i++ { total += i * 31 }
		_, _ = fmt.Fprintf(w, `{"service":"orders","work":%q,"checksum":%d}`, work, total)
	default:
		http.NotFound(w, r)
	}
})
