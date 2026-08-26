package analyzer

import (
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestAnalyzeFixtureBuildsUnifiedCodeModel(t *testing.T) {
	_, currentFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("cannot locate analyzer test")
	}
	fixture := filepath.Join(filepath.Dir(currentFile), "../../../../fixtures/go-sample")

	model, err := Analyze(fixture)
	if err != nil {
		t.Fatalf("analyze fixture: %v", err)
	}

	if model.Language != "go" {
		t.Fatalf("language: got %q, want %q", model.Language, "go")
	}
	if len(model.Packages) != 2 {
		t.Fatalf("packages: got %d, want 2", len(model.Packages))
	}
	for _, pkg := range model.Packages {
		for _, file := range pkg.Files {
			if !strings.HasSuffix(file, ".go") {
				t.Errorf("package %s contains non-Go file %q", pkg.ID, file)
			}
		}
	}

	createUser := findSymbol(model, "example.com/go-sample/internal/user.CreateUser")
	if createUser == nil {
		t.Fatal("CreateUser symbol was not found")
	}
	if createUser.Source.File != "internal/user/user.go" || createUser.Source.StartLine != 31 || createUser.Source.EndLine != 48 {
		t.Fatalf("CreateUser source: got %#v, want internal/user/user.go:31-48", createUser.Source)
	}

	mainCall := findCall(model, "example.com/go-sample.main", "example.com/go-sample/internal/user.CreateUser")
	if mainCall == nil {
		t.Fatal("main -> CreateUser call was not found")
	}

	databaseEffect := findEffect(model, createUser.ID, "database")
	if databaseEffect == nil || databaseEffect.Detail == "" {
		t.Fatalf("database effect on db.Save was not found: %#v", model.Effects)
	}
	if findEffect(model, createUser.ID, "other") == nil {
		t.Fatal("goroutine effect was not found")
	}
	if findEffect(model, createUser.ID, "queue") == nil {
		t.Fatal("queue effect was not found")
	}
}

func findSymbol(model UnifiedCodeModel, id string) *Symbol {
	for index := range model.Symbols {
		if model.Symbols[index].ID == id {
			return &model.Symbols[index]
		}
	}
	return nil
}

func findCall(model UnifiedCodeModel, from string, to string) *Call {
	for index := range model.Calls {
		if model.Calls[index].From == from && model.Calls[index].To == to {
			return &model.Calls[index]
		}
	}
	return nil
}

func findEffect(model UnifiedCodeModel, symbol string, kind string) *ExternalEffect {
	for index := range model.Effects {
		if model.Effects[index].Symbol == symbol && model.Effects[index].Kind == kind {
			return &model.Effects[index]
		}
	}
	return nil
}
