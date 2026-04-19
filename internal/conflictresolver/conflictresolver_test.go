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

func TestRedQueenStrategyBaseFallback(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyBaseFallback))

	content := `<<<<<<< HEAD
ours content
=======
theirs content
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

	if resolved != "ours content\n" {
		t.Errorf("expected 'ours content\\n', got '%s'", resolved)
	}
}

func TestRedQueenStrategyBaseFallbackSameContent(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyBaseFallback))

	content := `<<<<<<< HEAD
same content
=======
same content
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

	if resolved != "same content\n" {
		t.Errorf("expected 'same content\\n', got '%s'", resolved)
	}
}

func TestRedQueenStrategyBaseFallbackEmptyOurs(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyBaseFallback))

	content := `<<<<<<< HEAD

=======
theirs content
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

	if resolved != "theirs content\n" {
		t.Errorf("expected 'theirs content\\n', got '%s'", resolved)
	}
}

func TestRedQueenStrategyBaseFallbackEmptyTheirs(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyBaseFallback))

	content := `<<<<<<< HEAD
ours content
=======
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

	if resolved != "ours content\n" {
		t.Errorf("expected 'ours content\\n', got '%s'", resolved)
	}
}

func TestRedQueenResolveWithStrategy(t *testing.T) {
	content := `<<<<<<< HEAD
ours line
=======
theirs line
>>>>>>> branch
`

	r := NewResolver()
	resolved, err := r.ResolveWithStrategy(content, StrategyPreferOurs)
	if err != nil {
		t.Fatalf("ResolveWithStrategy failed: %v", err)
	}

	if resolved != "ours line\n" {
		t.Errorf("expected 'ours line\\n', got '%s'", resolved)
	}
}

func TestRedQueenResolveWithStrategyTheirs(t *testing.T) {
	content := `<<<<<<< HEAD
ours line
=======
theirs line
>>>>>>> branch
`

	r := NewResolver()
	resolved, err := r.ResolveWithStrategy(content, StrategyPreferTheirs)
	if err != nil {
		t.Fatalf("ResolveWithStrategy failed: %v", err)
	}

	if resolved != "theirs line\n" {
		t.Errorf("expected 'theirs line\\n', got '%s'", resolved)
	}
}

func TestRedQueenResolveWithStrategyInvalid(t *testing.T) {
	content := `<<<<<<< HEAD
ours line
=======
theirs line
>>>>>>> branch
`

	r := NewResolver()
	_, err := r.ResolveWithStrategy(content, "invalid-strategy")
	if err == nil {
		t.Fatalf("expected error for invalid strategy, got nil")
	}
}

func TestRedQueenAutoResolve(t *testing.T) {
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

	if err := AutoResolve(tmpfile); err != nil {
		t.Fatalf("AutoResolve failed: %v", err)
	}

	result, err := os.ReadFile(tmpfile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	if string(result) != "package main\n\nfunc ours() {}\n" {
		t.Errorf("unexpected result: '%s'", string(result))
	}
}

func TestRedQueenAutoResolveWithStrategy(t *testing.T) {
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

	if err := AutoResolve(tmpfile, StrategyPreferTheirs); err != nil {
		t.Fatalf("AutoResolve failed: %v", err)
	}

	result, err := os.ReadFile(tmpfile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	if string(result) != "package main\n\nfunc theirs() {}\n" {
		t.Errorf("unexpected result: '%s'", string(result))
	}
}

func TestRedQueenEmptyContent(t *testing.T) {
	r := NewResolver()
	_, err := r.ParseConflicts("")
	if err != nil {
		t.Fatalf("ParseConflicts failed on empty content: %v", err)
	}
}

func TestRedQueenNoConflicts(t *testing.T) {
	r := NewResolver()
	content := "no conflict markers here"
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 0 {
		t.Errorf("expected 0 conflicts, got %d", len(conflicts))
	}
}

func TestRedQueenOnlyStartMarker(t *testing.T) {
	r := NewResolver()
	content := "<<<<<<< HEAD"
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Errorf("BUG FOUND: unterminated start marker incorrectly parsed as conflict, got %d", len(conflicts))
	}
}

func TestRedQueenOnlyMidMarker(t *testing.T) {
	r := NewResolver()
	content := "======="
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 0 {
		t.Errorf("expected 0 conflicts for lone mid marker, got %d", len(conflicts))
	}
}

func TestRedQueenOnlyEndMarker(t *testing.T) {
	r := NewResolver()
	content := ">>>>>>> branch"
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 0 {
		t.Errorf("expected 0 conflicts for lone end marker, got %d", len(conflicts))
	}
}

func TestRedQueenMissingEndMarker(t *testing.T) {
	r := NewResolver()
	content := `<<<<<<< HEAD
ours content
=======
theirs content
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Errorf("BUG FOUND: missing end marker incorrectly parsed as conflict, got %d", len(conflicts))
	}
}

func TestRedQueenMissingMidMarker(t *testing.T) {
	r := NewResolver()
	content := `<<<<<<< HEAD
ours content
theirs content
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Errorf("BUG FOUND: missing mid marker incorrectly parsed as conflict, got %d", len(conflicts))
	}
}

func TestRedQueenConflictAtStart(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
first
=======
second
>>>>>>> branch
after
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict, got %d", len(conflicts))
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := "first\nafter\n"
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestRedQueenConflictAtEnd(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `before
<<<<<<< HEAD
first
=======
second
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict, got %d", len(conflicts))
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := "before\nfirst\n"
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestRedQueenConsecutiveConflicts(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
a1
=======
b1
>>>>>>> branch
<<<<<<< HEAD
a2
=======
b2
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 2 {
		t.Fatalf("expected 2 conflicts, got %d", len(conflicts))
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := "a1\na2\n"
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestRedQueenEmptyOursSection(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
=======
theirs only
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict, got %d", len(conflicts))
	}
	if conflicts[0].Ours != "" {
		t.Errorf("expected empty ours, got '%s'", conflicts[0].Ours)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if resolved != "theirs only\n" {
		t.Errorf("BUG FOUND: prefer ours with empty ours should return theirs, got '%s'", resolved)
	}
}

func TestRedQueenEmptyTheirsSection(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
ours only
=======
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict, got %d", len(conflicts))
	}
	if conflicts[0].Theirs != "" {
		t.Errorf("expected empty theirs, got '%s'", conflicts[0].Theirs)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if resolved != "ours only\n" {
		t.Errorf("expected 'ours only\\n', got '%s'", resolved)
	}
}

func TestRedQueenValidationDisabled(t *testing.T) {
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
		t.Fatalf("ResolveFile with validation disabled failed: %v", err)
	}
}

func TestRedQueenValidationEnabledGo(t *testing.T) {
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

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(true))
	err := r.ResolveFile(tmpfile, StrategyPreferOurs)
	if err == nil {
		t.Fatalf("expected validation to fail for invalid Go, got nil")
	}
}

func TestRedQueenStrategyManual(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyManual))
	content := `<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	_, err = r.Resolve(conflicts, content, content)
	if err == nil {
		t.Fatalf("expected error for StrategyManual, got nil")
	}
}

func TestRedQueenResolveWithStrategyBaseFallback(t *testing.T) {
	content := `<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch
`

	r := NewResolver()
	resolved, err := r.ResolveWithStrategy(content, StrategyBaseFallback)
	if err != nil {
		t.Fatalf("ResolveWithStrategy failed: %v", err)
	}

	if resolved != "ours\n" {
		t.Errorf("expected 'ours\\n', got '%s'", resolved)
	}
}

func TestRedQueenResolveFileWithValidation(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.rs")

	content := `fn ours() {}
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(false))
	if err := r.ResolveFile(tmpfile, StrategyPreferOurs); err != nil {
		t.Fatalf("ResolveFile failed: %v", err)
	}
}

func TestRedQueenNoConflictMarkersInContent(t *testing.T) {
	r := NewResolver()
	content := `func normal() {
    fmt.Println("not a conflict")
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

func TestRedQueenContentThatLooksLikeConflict(t *testing.T) {
	r := NewResolver()
	content := `func example() {
    fmt.Println("<<<<<<< should not be a conflict")
    fmt.Println("=======")
    fmt.Println(">>>>>>> end")
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

func TestRedQueenResolveFileNoConflicts(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.go")

	content := `package main

func main() {
}
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(true))
	if err := r.ResolveFile(tmpfile, StrategyPreferOurs); err != nil {
		t.Fatalf("ResolveFile with no conflicts should succeed: %v", err)
	}
}

func TestRedQueenAutoResolveNoConflicts(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.go")

	content := `package main

func main() {
}
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	if err := AutoResolve(tmpfile); err != nil {
		t.Fatalf("AutoResolve with no conflicts should succeed: %v", err)
	}
}

func TestRedQueenBugResolveInvalidStrategy(t *testing.T) {
	r := NewResolver()
	content := `<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch
`

	_, err := r.ResolveWithStrategy(content, "invalid-strategy")
	if err == nil {
		t.Errorf("BUG FOUND: ResolveWithStrategy with unknown strategy should return error, but got nil")
	}
}

func TestRedQueenMultipleConflictsOffsetCalculation(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
a
=======
b
>>>>>>> branch
text1
<<<<<<< HEAD
c
=======
d
>>>>>>> branch
text2
<<<<<<< HEAD
e
=======
f
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 3 {
		t.Fatalf("expected 3 conflicts, got %d", len(conflicts))
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := "a\ntext1\nc\ntext2\ne\n"
	if resolved != expected {
		t.Errorf("expected '%s', got '%s'", expected, resolved)
	}
}

func TestRedQueenResolveFileNonGoFile(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.txt")

	content := `<<<<<<< HEAD
ours content
=======
theirs content
>>>>>>> branch
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(true))
	if err := r.ResolveFile(tmpfile, StrategyPreferOurs); err != nil {
		t.Fatalf("ResolveFile for non-Go file should not validate: %v", err)
	}
}

func TestRedQueenResolveFileRustFile(t *testing.T) {
	tmpdir := t.TempDir()
	tmpfile := filepath.Join(tmpdir, "test.rs")

	content := `<<<<<<< HEAD
fn ours() {}
=======
fn theirs() {}
>>>>>>> branch
`
	if err := os.WriteFile(tmpfile, []byte(content), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	r := NewResolver(WithStrategy(StrategyPreferOurs), WithValidation(false))
	if err := r.ResolveFile(tmpfile, StrategyPreferOurs); err != nil {
		t.Fatalf("ResolveFile for Rust file failed: %v", err)
	}
}

func TestRedQueenBugMalformedConflictMarkers(t *testing.T) {
	r := NewResolver()

	testCases := []struct {
		name    string
		content string
	}{
		{"no space after start", "<<<<<<<HEAD\nours\n=======\ntheirs\n>>>>>>>branch"},
		{"trailing space on markers", "<<<<<<< HEAD \nours\n=======\ntheirs\n>>>>>>> branch "},
		{"tab after start", "<<<<<<<\tHEAD\nours\n=======\ntheirs\n>>>>>>> branch"},
		{"mixed markers", "<<<<<<< HEAD\nours\n========\ntheirs\n>>>>>>> branch"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			conflicts, err := r.ParseConflicts(tc.content)
			if err != nil {
				t.Fatalf("ParseConflicts failed: %v", err)
			}
			if len(conflicts) != 0 {
				t.Errorf("BUG FOUND: malformed markers parsed as valid conflict, got %d", len(conflicts))
			}
		})
	}
}

func TestRedQueenOverlappingConflictMarkers(t *testing.T) {
	r := NewResolver()

	content := `<<<<<<< HEAD
<<<<<<< HEAD
=======
inner
>>>>>>> branch
=======
outer
>>>>>>> branch
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}
	if len(conflicts) != 1 {
		t.Fatalf("expected 1 conflict (outer), got %d", len(conflicts))
	}
}

func TestRedQueenSingleLineConflict(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `<<<<<<< HEAD
single
=======
other
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

	if resolved != "single\n" {
		t.Errorf("expected 'single\\n', got '%s'", resolved)
	}
}

func TestRedQueenVeryLongLine(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	longLineBytes := make([]byte, 10000)
	for i := range longLineBytes {
		longLineBytes[i] = 'a'
	}
	longLine := string(longLineBytes)
	content := "<<<<<<< HEAD\n" + longLine + "\n=======\nshort\n>>>>>>> branch\n"

	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if len(resolved) < 10000 {
		t.Errorf("resolved content too short: %d", len(resolved))
	}
}

func TestRedQueenWhitespaceOnlyContent(t *testing.T) {
	r := NewResolver()
	_, err := r.ParseConflicts("   \n\t\n   ")
	if err != nil {
		t.Fatalf("ParseConflicts failed on whitespace: %v", err)
	}
}

func TestRedQueenResolvePreservesNonConflictContent(t *testing.T) {
	r := NewResolver(WithStrategy(StrategyPreferOurs))
	content := `func before() {}
<<<<<<< HEAD
ours
=======
theirs
>>>>>>> branch
func after() {}
`
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		t.Fatalf("ParseConflicts failed: %v", err)
	}

	resolved, err := r.Resolve(conflicts, content, content)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !contains(resolved, "func before() {}") || !contains(resolved, "func after() {}") {
		t.Errorf("non-conflict content not preserved: '%s'", resolved)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && findSubstring(s, substr)
}

func findSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
