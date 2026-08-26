package user

import (
    "context"
    "fmt"
    "net/http"
)

type DB struct{}

func (db *DB) Save(ctx context.Context, name string) error {
    _ = ctx
    _ = name
    return nil
}

func parseName(raw string) string {
    return raw
}

func validateName(name string) bool {
    return name != ""
}

func sendEmail(name string) error {
    _, err := http.Post("https://example.test/welcome", "text/plain", nil)
    _ = name
    return err
}

func CreateUser(ctx context.Context, db *DB, raw string) error {
    name := parseName(raw)
    if !validateName(name) {
        return fmt.Errorf("invalid user name")
    }
    if err := db.Save(ctx, name); err != nil {
        return err
    }
    notifications := make(chan string, 1)
    go func() {
        _ = sendEmail(name)
    }()
    select {
    case notifications <- name:
    default:
    }
    return nil
}
