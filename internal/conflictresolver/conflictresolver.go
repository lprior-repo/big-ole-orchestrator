package conflictresolver

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

type Conflict struct {
	StartLine  int
	EndLine    int
	Ours       string
	Theirs     string
	Base       string
	OurLabel   string
	TheirLabel string
}

type ResolutionStrategy string

const (
	StrategyPreferOurs   ResolutionStrategy = "prefer-ours"
	StrategyPreferTheirs ResolutionStrategy = "prefer-theirs"
	StrategyBaseFallback ResolutionStrategy = "base-fallback"
	StrategyManual       ResolutionStrategy = "manual"
)

type Resolver struct {
	strategy ResolutionStrategy
	validate bool
}

type ResolverOption func(*Resolver)

func WithStrategy(s ResolutionStrategy) ResolverOption {
	return func(r *Resolver) {
		r.strategy = s
	}
}

func WithValidation(enabled bool) ResolverOption {
	return func(r *Resolver) {
		r.validate = enabled
	}
}

func NewResolver(opts ...ResolverOption) *Resolver {
	r := &Resolver{
		strategy: StrategyPreferOurs,
		validate: true,
	}
	for _, opt := range opts {
		opt(r)
	}
	return r
}

var conflictPattern = regexp.MustCompile(`^<<<<<<<\s+(.+)\n([\s\S]*?)^=======\n([\s\S]*?)^>>>>>>>\s+(.+)$`)
var conflictStartPattern = regexp.MustCompile(`^<<<<<<<\s+(.+)$`)
var conflictMidPattern = regexp.MustCompile(`^=======$`)
var conflictEndPattern = regexp.MustCompile(`^>>>>>>>\s+(.+)$`)

func (r *Resolver) ParseConflicts(content string) ([]Conflict, error) {
	var conflicts []Conflict
	lines := strings.Split(content, "\n")

	i := 0
	for i < len(lines) {
		if conflictStartPattern.MatchString(lines[i]) {
			ourLabel := conflictStartPattern.FindStringSubmatch(lines[i])[1]
			i++

			var ours []string
			for i < len(lines) && !conflictMidPattern.MatchString(lines[i]) {
				ours = append(ours, lines[i])
				i++
			}
			i++

			var theirs []string
			for i < len(lines) && !conflictEndPattern.MatchString(lines[i]) {
				theirs = append(theirs, lines[i])
				i++
			}

			var theirLabel string
			if i < len(lines) && conflictEndPattern.MatchString(lines[i]) {
				theirLabel = conflictEndPattern.FindStringSubmatch(lines[i])[1]
				i++
			}

			conflicts = append(conflicts, Conflict{
				StartLine:  i - len(theirs) - len(ours) - 3,
				EndLine:    i,
				Ours:       strings.Join(ours, "\n"),
				Theirs:     strings.Join(theirs, "\n"),
				OurLabel:   ourLabel,
				TheirLabel: theirLabel,
			})
		} else {
			i++
		}
	}

	return conflicts, nil
}

func (r *Resolver) Resolve(conflicts []Conflict, ours, theirs string) (string, error) {
	switch r.strategy {
	case StrategyPreferOurs:
		return r.resolvePreferOurs(conflicts, ours)
	case StrategyPreferTheirs:
		return r.resolvePreferTheirs(conflicts, theirs)
	case StrategyBaseFallback:
		return r.resolveBaseFallback(conflicts, ours, theirs)
	default:
		return "", fmt.Errorf("unknown strategy: %s", r.strategy)
	}
}

func (r *Resolver) resolvePreferOurs(conflicts []Conflict, content string) (string, error) {
	result := content
	offset := 0

	for _, c := range conflicts {
		start := c.StartLine + offset
		end := c.EndLine + offset

		lines := strings.Split(result, "\n")
		if start < 0 || start >= len(lines) || end < 0 || end > len(lines) {
			continue
		}

		resolved := append([]string{}, lines[:start]...)
		resolved = append(resolved, strings.Split(c.Ours, "\n")...)
		resolved = append(resolved, lines[end:]...)

		result = strings.Join(resolved, "\n")
		offset += len(strings.Split(c.Ours, "\n")) - (end - start)
	}

	return result, nil
}

func (r *Resolver) resolvePreferTheirs(conflicts []Conflict, content string) (string, error) {
	result := content
	offset := 0

	for _, c := range conflicts {
		start := c.StartLine + offset
		end := c.EndLine + offset

		lines := strings.Split(result, "\n")
		if start < 0 || start >= len(lines) || end < 0 || end > len(lines) {
			continue
		}

		resolved := append([]string{}, lines[:start]...)
		resolved = append(resolved, strings.Split(c.Theirs, "\n")...)
		resolved = append(resolved, lines[end:]...)

		result = strings.Join(resolved, "\n")
		offset += len(strings.Split(c.Theirs, "\n")) - (end - start)
	}

	return result, nil
}

func (r *Resolver) resolveBaseFallback(conflicts []Conflict, ours, theirs string) (string, error) {
	result := ours
	offset := 0

	for _, c := range conflicts {
		start := c.StartLine + offset
		end := c.EndLine + offset

		lines := strings.Split(result, "\n")
		if start < 0 || start >= len(lines) || end < 0 || end > len(lines) {
			continue
		}

		resolved := append([]string{}, lines[:start]...)
		if c.Ours == c.Theirs {
			resolved = append(resolved, strings.Split(c.Ours, "\n")...)
		} else if c.Ours == "" {
			resolved = append(resolved, strings.Split(c.Theirs, "\n")...)
		} else if c.Theirs == "" {
			resolved = append(resolved, strings.Split(c.Ours, "\n")...)
		} else {
			resolved = append(resolved, strings.Split(c.Ours, "\n")...)
		}
		resolved = append(resolved, lines[end:]...)

		result = strings.Join(resolved, "\n")
	}

	return result, nil
}

func (r *Resolver) ResolveFile(path string, strategy ResolutionStrategy) error {
	content, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("failed to read file: %w", err)
	}

	conflicts, err := r.ParseConflicts(string(content))
	if err != nil {
		return fmt.Errorf("failed to parse conflicts: %w", err)
	}

	if len(conflicts) == 0 {
		return nil
	}

	resolved, err := r.ResolveWithStrategy(string(content), strategy)
	if err != nil {
		return fmt.Errorf("failed to resolve conflicts: %w", err)
	}

	if r.validate {
		if err := r.validateResult(path, resolved); err != nil {
			return fmt.Errorf("validation failed: %w", err)
		}
	}

	if err := os.WriteFile(path, []byte(resolved), 0644); err != nil {
		return fmt.Errorf("failed to write file: %w", err)
	}

	return nil
}

func (r *Resolver) ResolveWithStrategy(content string, strategy ResolutionStrategy) (string, error) {
	conflicts, err := r.ParseConflicts(content)
	if err != nil {
		return "", err
	}

	switch strategy {
	case StrategyPreferOurs:
		return r.resolvePreferOurs(conflicts, content)
	case StrategyPreferTheirs:
		return r.resolvePreferTheirs(conflicts, content)
	case StrategyBaseFallback:
		return r.resolveBaseFallback(conflicts, content, content)
	default:
		return "", fmt.Errorf("unknown strategy: %s", strategy)
	}
}

func (r *Resolver) validateResult(path string, content string) error {
	ext := strings.ToLower(path)

	if strings.HasSuffix(ext, ".go") {
		return r.validateGo(content, path)
	} else if strings.HasSuffix(ext, ".rs") {
		return r.validateRust(content, path)
	}

	return nil
}

func (r *Resolver) validateGo(content, path string) error {
	tmpfile, err := os.CreateTemp("", "*.go")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	defer os.Remove(tmpfile.Name())

	if _, err := tmpfile.Write([]byte(content)); err != nil {
		return fmt.Errorf("failed to write temp file: %w", err)
	}
	tmpfile.Close()

	cmd := exec.Command("go", "build", "-o", os.DevNull, tmpfile.Name())
	cmd.Dir = r.findModuleRoot(path)

	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("go build failed: %s", stderr.String())
	}

	return nil
}

func (r *Resolver) validateRust(content, path string) error {
	tmpfile, err := os.CreateTemp("", "*.rs")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	defer os.Remove(tmpfile.Name())

	if _, err := tmpfile.Write([]byte(content)); err != nil {
		return fmt.Errorf("failed to write temp file: %w", err)
	}
	tmpfile.Close()

	cmd := exec.Command("cargo", "check", "--message-format=short")
	cmd.Dir = r.findModuleRoot(path)

	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("cargo check failed: %s", stderr.String())
	}

	return nil
}

func (r *Resolver) findModuleRoot(path string) string {
	dir := path
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir
		}
		if _, err := os.Stat(filepath.Join(dir, "Cargo.toml")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return "."
}

func AutoResolve(path string, strategies ...ResolutionStrategy) error {
	r := NewResolver()
	if len(strategies) > 0 {
		r.strategy = strategies[0]
	}

	content, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("failed to read file: %w", err)
	}

	conflicts, err := r.ParseConflicts(string(content))
	if err != nil {
		return fmt.Errorf("failed to parse conflicts: %w", err)
	}

	if len(conflicts) == 0 {
		return nil
	}

	resolved, err := r.Resolve(conflicts, string(content), string(content))
	if err != nil {
		return fmt.Errorf("failed to resolve conflicts: %w", err)
	}

	if err := os.WriteFile(path, []byte(resolved), 0644); err != nil {
		return fmt.Errorf("failed to write file: %w", err)
	}

	return nil
}
