package conflictresolver

import (
	"os"
	"path/filepath"
	"testing"
)

func TestParseConflicts(t *testing.T) {
	r := NewResolver()

	content := `<<<<<<< HEAD
ours content
=======
theirs content
>>>>>>> branch

outside conflict

<<<<<<< HEAD
second ours
=======
second theirs
>>>>>>> feature
`

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	if len(conflicts) != 2 {
		t.Fatalf("expected 2 conflicts, got %d", len(conflicts))
	}

	if conflicts[0].OurLabel != "HEAD" {
		t.Errorf("expected our label 'HEAD', got '%s'", conflicts[0].OurLabel)
	}
	if conflicts[0].TheirLabel != "branch" {
		t.Errorf("expected their label 'branch', got '%s'", conflicts[0].TheirLabel)
	}
	if conflicts[0].Ours != "ours content" {
		t.Errorf("expected ours 'ours content', got '%s'", conflicts[0].Ours)
	}
	if conflicts[0].Theirs != "theirs content" {
		t.Errorf("expected theirs 'theirs content', got '%s'", conflicts[0].Theirs)
	}
}

func TestResolvePreferOurs(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))

	content := `<<<<<<< HEAD
ours line
=======
theirs line
>>>>>>> branch
`

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := `ours line
`
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestResolvePreferTheirs(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferTheirs))

	content := `<<<<<<< HEAD
ours line
=======
theirs line
>>>>>>> branch
`

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := `theirs line
`
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestResolveFile(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.go")

	content := `package main

<<<<<<< HEAD
func ours() {}
=======
func theirs() {}
>>>>>>> branch
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(false))
	if err := r.ResolveFile(tmpfile, StrategyPreferOurs); err != nil {
		t.Fatalf("ResolveFile failed: %v", err)
	}

	result, err := os.ReadFile(tmpfile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	if string(result) != "package main\n\nfunc ours() {}\n" {
		t.Errorf("unexpected result: '%s'", string(result))
	}
}

func TestNoConflicts(t *testing.T) {
	r := NewResolver()

	content := `package main

func main() {
}
`

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	if len(conflicts) != 0 {
		t.Errorf("expected 0 conflicts, got %d", len(conflicts))
	}
}

func TestMultiLineConflict(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))

	content := `<<<<<<< HEAD
line1
line2
line3
=======
lineA
lineB
lineC
>>>>>>> branch
`

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict, got %d", len(conflicts))
	}

	if conflicts[0].Ours != "line1\nline2\nline3" {
		t.Errorf("expected ours 'line1\\nline2\\nline3', got '%s'", conflicts[0].Ours)
	}
}
