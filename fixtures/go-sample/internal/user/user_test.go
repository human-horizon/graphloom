package user

import "testing"

func TestCreateUser(t *testing.T) {
    if !validateName("Anya") {
        t.Fatal("expected a valid name")
    }
}
