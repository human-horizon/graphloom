package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"graphloom/analyzers/go/internal/analyzer"
)

func main() {
	dir := flag.String("dir", "", "Go project directory")
	flag.Parse()
	if *dir == "" {
		fmt.Fprintln(os.Stderr, "graphloom-analyze: -dir is required")
		os.Exit(2)
	}

	model, err := analyzer.Analyze(*dir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "graphloom-analyze: %v\n", err)
		os.Exit(1)
	}
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(model); err != nil {
		fmt.Fprintf(os.Stderr, "graphloom-analyze: encode JSON: %v\n", err)
		os.Exit(1)
	}
}
