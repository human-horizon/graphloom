package main

import (
    "context"

    "example.com/go-sample/internal/user"
)

func main() {
    database := &user.DB{}
    _ = user.CreateUser(context.Background(), database, "Anya")
}
