bead_id: vo-acb
bead_title: vo-types: define all semantic newtypes
phase: state-1.5-retry-1
updated_at: 2026-03-27T05:12:00Z

# Test Plan: vo-types: define all semantic newtypes

## Summary

- Behaviors identified: 22 categories, 209 individual test scenarios
  - Section 3 BDD scenarios: 183 (143 original + 40 promoted from Section 8 matrix)
  - Section 4 proptest invariants: 26
- Trophy allocation: ~162 unit / ~43 integration / ~0 e2e / ~3 static
- Proptest invariants: 26
- Fuzz targets: 14
- Kani harnesses: 8
- Mutation threshold: >= 90% kill rate

### Rationale for Trophy Ratio Deviation

The standard Testing Trophy targets ~60% integration / ~30% unit / ~5% e2e / ~5% static.
This crate is **entirely Calc layer** — pure functions, zero I/O, zero component boundaries.
There are no databases, no network calls, no async runtimes, no external services to integrate.
The only "integration" surface is serde (Serialize/Deserialize), which accounts for ~27% of scenarios.
All `parse()` constructors, accessors, Display impls, and trait derivations are pure unit tests.
There is no meaningful E2E layer for a type-definition crate.
Static analysis covers clippy, cargo-deny, and the compile-time guarantee that no newtype derives `Default`.

```
         [E2E]           0%  -- no user-facing workflow in this crate
    [Integration]       27%  -- serde round-trip, serde rejection, ParseError Display
    [Unit / Calc]       68%  -- all parse() constructors, accessors, Display, traits, edge cases
  [Static Analysis]      5%  -- clippy, cargo-deny, no-Default compile-time gate
```

---

## 1. Behavior Inventory

### parse() constructors (14 newtypes)

1. InstanceId accepts valid 26-char Crockford Base32 ULID when input is well-formed
2. InstanceId rejects empty input with Empty error when string is empty
3. InstanceId rejects non-ULID with InvalidFormat error when length is not 26
4. InstanceId rejects invalid chars with InvalidFormat error when chars are not Crockford Base32
5. InstanceId rejects malformed ULID with InvalidFormat error when ulid crate validation fails
6. WorkflowName accepts valid identifier when chars are [a-zA-Z0-9_-], non-empty, <=128 chars, no leading/trailing - or _
7. WorkflowName rejects empty input with Empty error when string is empty
8. WorkflowName rejects invalid characters with InvalidCharacters error when chars outside [a-zA-Z0-9_-]
9. WorkflowName rejects too-long input with ExceedsMaxLength error when length > 128
10. WorkflowName rejects leading hyphen with BoundaryViolation error when string starts with "-"
11. WorkflowName rejects leading underscore with BoundaryViolation error when string starts with "_"
12. WorkflowName rejects trailing hyphen with BoundaryViolation error when string ends with "-"
13. WorkflowName rejects trailing underscore with BoundaryViolation error when string ends with "_"
14. NodeName accepts valid identifier when chars are [a-zA-Z0-9_-], non-empty, <=128 chars, no leading/trailing - or _
15. NodeName rejects empty input with Empty error when string is empty
16. NodeName rejects invalid characters with InvalidCharacters error when chars outside [a-zA-Z0-9_-]
17. NodeName rejects too-long input with ExceedsMaxLength error when length > 128
18. NodeName rejects leading hyphen with BoundaryViolation error when string starts with "-"
19. NodeName rejects leading underscore with BoundaryViolation error when string starts with "_"
20. NodeName rejects trailing hyphen with BoundaryViolation error when string ends with "-"
21. NodeName rejects trailing underscore with BoundaryViolation error when string ends with "_"
22. BinaryHash accepts valid lowercase hex when chars are [0-9a-f], even length, >= 8 chars
23. BinaryHash rejects empty input with Empty error when string is empty
24. BinaryHash rejects uppercase hex with InvalidCharacters error when chars include [A-F]
25. BinaryHash rejects non-hex characters with InvalidCharacters error when chars outside [0-9a-fA-F]
26. BinaryHash rejects odd-length input with InvalidFormat error when length is odd
27. BinaryHash rejects too-short input with InvalidFormat error when length < 8
28. SequenceNumber accepts valid nonzero decimal when string parses to u64 > 0
29. SequenceNumber rejects non-integer with NotAnInteger error when string is not a valid u64
30. SequenceNumber rejects zero with ZeroValue error when parsed value is 0
31. EventVersion accepts valid nonzero decimal when string parses to u64 > 0
32. EventVersion rejects non-integer with NotAnInteger error when string is not a valid u64
33. EventVersion rejects zero with ZeroValue error when parsed value is 0
34. AttemptNumber accepts valid nonzero decimal when string parses to u64 > 0
35. AttemptNumber rejects non-integer with NotAnInteger error when string is not a valid u64
36. AttemptNumber rejects zero with ZeroValue error when parsed value is 0
37. TimerId accepts non-empty string when length <= 256
38. TimerId rejects empty input with Empty error when string is empty
39. TimerId rejects too-long input with ExceedsMaxLength error when length > 256
40. IdempotencyKey accepts non-empty string when length <= 1024
41. IdempotencyKey rejects empty input with Empty error when string is empty
42. IdempotencyKey rejects too-long input with ExceedsMaxLength error when length > 1024
43. TimeoutMs accepts valid nonzero decimal when string parses to u64 > 0
44. TimeoutMs rejects non-integer with NotAnInteger error when string is not a valid u64
45. TimeoutMs rejects zero with ZeroValue error when parsed value is 0
46. DurationMs accepts valid decimal including zero when string parses to u64 >= 0
47. DurationMs rejects non-integer with NotAnInteger error when string is not a valid u64
48. TimestampMs accepts valid decimal including zero when string parses to u64 >= 0
49. TimestampMs rejects non-integer with NotAnInteger error when string is not a valid u64
50. FireAtMs accepts valid decimal including zero when string parses to u64 >= 0
51. FireAtMs rejects non-integer with NotAnInteger error when string is not a valid u64
52. MaxAttempts accepts valid nonzero decimal when string parses to u64 > 0
53. MaxAttempts rejects non-integer with NotAnInteger error when string is not a valid u64
54. MaxAttempts rejects zero with ZeroValue error when parsed value is 0

### Accessor methods (14)

55. InstanceId::as_str returns inner string borrowing from the newtype when called on valid instance
56. WorkflowName::as_str returns inner string borrowing from the newtype when called on valid instance
57. NodeName::as_str returns inner string borrowing from the newtype when called on valid instance
58. BinaryHash::as_str returns inner string borrowing from the newtype when called on valid instance
59. TimerId::as_str returns inner string borrowing from the newtype when called on valid instance
60. IdempotencyKey::as_str returns inner string borrowing from the newtype when called on valid instance
61. SequenceNumber::as_u64 returns inner value when called on valid instance
62. EventVersion::as_u64 returns inner value when called on valid instance
63. AttemptNumber::as_u64 returns inner value when called on valid instance
64. TimeoutMs::as_u64 returns inner value when called on valid instance
65. DurationMs::as_u64 returns inner value when called on valid instance
66. TimestampMs::as_u64 returns inner value when called on valid instance
67. FireAtMs::as_u64 returns inner value when called on valid instance
68. MaxAttempts::as_u64 returns inner value when called on valid instance

### Unsafe/bypass constructors (5 newtypes with new_unchecked)

69. SequenceNumber::new_unchecked constructs from nonzero u64 when value > 0
70. SequenceNumber::new_unchecked panics when value is 0
71. EventVersion::new_unchecked constructs from nonzero u64 when value > 0
72. EventVersion::new_unchecked panics when value is 0
73. AttemptNumber::new_unchecked constructs from nonzero u64 when value > 0
74. AttemptNumber::new_unchecked panics when value is 0
75. TimeoutMs::new_unchecked constructs from nonzero u64 when value > 0
76. TimeoutMs::new_unchecked panics when value is 0
77. MaxAttempts::new_unchecked constructs from nonzero u64 when value > 0
78. MaxAttempts::new_unchecked panics when value is 0

### Conversion methods (7)

79. TimeoutMs::to_duration returns correct std::time::Duration when called
80. DurationMs::to_duration returns correct std::time::Duration when called, including zero
81. TimestampMs::to_system_time returns UNIX_EPOCH when value is 0
82. TimestampMs::to_system_time returns correct SystemTime when value > 0
83. FireAtMs::to_system_time returns correct SystemTime when called
84. TimestampMs::now returns parseable TimestampMs when called
85. TimestampMs::now returns value approximately equal to current system time when called
86. FireAtMs::has_elapsed returns true when fire_at < now
87. FireAtMs::has_elapsed returns false when fire_at > now
88. FireAtMs::has_elapsed returns deterministic result when fire_at equals now
89. MaxAttempts::is_exhausted returns false when attempt < max
90. MaxAttempts::is_exhausted returns true when attempt >= max

### Display (14)

91. InstanceId Display output equals inner string (identity) when formatted
92. WorkflowName Display output equals inner string (identity) when formatted
93. NodeName Display output equals inner string (identity) when formatted
94. BinaryHash Display output equals inner string (identity) when formatted
95. TimerId Display output equals inner string (identity) when formatted
96. IdempotencyKey Display output equals inner string (identity) when formatted
97. SequenceNumber Display output equals decimal representation without padding when formatted
98. EventVersion Display output equals decimal representation without padding when formatted
99. AttemptNumber Display output equals decimal representation without padding when formatted
100. TimeoutMs Display output equals decimal representation without padding when formatted
101. DurationMs Display output equals decimal representation without padding when formatted
102. TimestampMs Display output equals decimal representation without padding when formatted
103. FireAtMs Display output equals decimal representation without padding when formatted
104. MaxAttempts Display output equals decimal representation without padding when formatted

### Round-trip (14)

105. InstanceId round-trips through parse(display(value)) when value is valid
106. WorkflowName round-trips through parse(display(value)) when value is valid
107. NodeName round-trips through parse(display(value)) when value is valid
108. BinaryHash round-trips through parse(display(value)) when value is valid
109. SequenceNumber round-trips through parse(display(value)) when value is valid
110. EventVersion round-trips through parse(display(value)) when value is valid
111. AttemptNumber round-trips through parse(display(value)) when value is valid
112. TimerId round-trips through parse(display(value)) when value is valid
113. IdempotencyKey round-trips through parse(display(value)) when value is valid
114. TimeoutMs round-trips through parse(display(value)) when value is valid
115. DurationMs round-trips through parse(display(value)) when value is valid
116. TimestampMs round-trips through parse(display(value)) when value is valid
117. FireAtMs round-trips through parse(display(value)) when value is valid
118. MaxAttempts round-trips through parse(display(value)) when value is valid

### Serde (28)

119-132. Serialize produces same string as Display for all 14 newtypes
133-146. Deserialize accepts valid input for all 14 newtypes
147-148. Deserialize rejects invalid string for InstanceId with serde error
149-150. Deserialize rejects invalid string for WorkflowName with serde error
151-152. Deserialize rejects invalid string for NodeName with serde error
153-154. Deserialize rejects invalid string for BinaryHash with serde error
155-156. Deserialize rejects invalid integer for SequenceNumber with serde error
157-158. Deserialize rejects invalid integer for EventVersion with serde error
159-160. Deserialize rejects invalid integer for AttemptNumber with serde error
161-162. Deserialize rejects invalid string for TimerId with serde error
163-164. Deserialize rejects invalid string for IdempotencyKey with serde error
165-166. Deserialize rejects invalid integer for TimeoutMs with serde error
167-168. Deserialize rejects invalid integer for DurationMs with serde error

(Note: serde rejection for TimestampMs, FireAtMs, MaxAttempts follows the same pattern as other integer types — covered in the combinatorial matrix.)

### Trait derivations (covered by proptest invariants — see Section 4)

- PartialEq/Eq, Hash, Clone, Copy (integer types), PartialOrd/Ord (integer types), Debug

### Integer edge cases (shared across 8 integer newtypes)

169. All integer newtypes accept leading zeros when input like "007"
170. All integer newtypes reject hex prefix with NotAnInteger when input starts with "0x"
171. All integer newtypes reject octal prefix with NotAnInteger when input starts with "0o"
172. All integer newtypes reject binary prefix with NotAnInteger when input starts with "0b"
173. All integer newtypes reject negative sign with NotAnInteger when input starts with "-"
174. All integer newtypes reject overflow with NotAnInteger when input exceeds u64::MAX
175. All integer newtypes reject whitespace with NotAnInteger when input contains spaces

### Whitespace preservation (shared across 6 string newtypes)

176. String newtypes reject leading whitespace with appropriate error when input starts with space
177. String newtypes reject trailing whitespace with appropriate error when input ends with space
178. String newtypes do NOT strip whitespace — caller is responsible for trimming
179. TimerId and IdempotencyKey accept trailing whitespace when input ends with space (opaque types preserve input)

### Additional boundary behaviors (promoted from Section 8 matrix)

180. InstanceId rejects long input with InvalidFormat error when length exceeds 26
181. InstanceId rejects leading whitespace with InvalidFormat error when input starts with space
182. InstanceId rejects trailing whitespace with InvalidFormat error when input ends with space
183. WorkflowName accepts single character when input is one valid char
184. WorkflowName accepts hyphen-only compound when input contains only alphanumeric and hyphens
185. WorkflowName accepts underscore-only compound when input contains only alphanumeric and underscores
186. WorkflowName accepts digits in identifier when input contains [0-9]
187. WorkflowName rejects null byte with InvalidCharacters error when input contains \x00
188. WorkflowName rejects unicode combining mark with InvalidCharacters error when input has composing characters
189. WorkflowName rejects whitespace-only with InvalidCharacters error when input is only spaces
190. NodeName accepts single character when input is one valid char
191. NodeName accepts hyphen-only compound when input contains only alphanumeric and hyphens
192. NodeName accepts underscore-only compound when input contains only alphanumeric and underscores
193. NodeName accepts digits in identifier when input contains [0-9]
194. NodeName rejects null byte with InvalidCharacters error when input contains \x00
195. BinaryHash accepts longer even-length hex when length is between 8 and 256
196. BinaryHash rejects mixed case with InvalidCharacters error when input has uppercase letters
197. BinaryHash rejects leading whitespace with InvalidCharacters error when input starts with space
198. BinaryHash rejects trailing whitespace with InvalidCharacters error when input ends with space
199. BinaryHash accepts all-zeros minimum when input is "00000000"
200. TimerId accepts single character when input is one char
201. TimerId accepts unicode when input has non-ASCII characters
202. IdempotencyKey accepts single character when input is one char
203. IdempotencyKey accepts unicode when input has non-ASCII characters
204. TimeoutMs accepts u64::MAX when input is "18446744073709551615"
205. TimestampMs accepts u64::MAX when input is "18446744073709551615"
206. FireAtMs accepts u64::MAX when input is "18446744073709551615"
207. MaxAttempts accepts u64::MAX when input is "18446744073709551615"
208. DurationMs rejects negative with NotAnInteger when input starts with "-"
209. TimestampMs rejects negative with NotAnInteger when input starts with "-"
210. FireAtMs rejects negative with NotAnInteger when input starts with "-"
211. TimeoutMs rejects negative with NotAnInteger when input starts with "-"
212. MaxAttempts rejects negative with NotAnInteger when input starts with "-"
213. SequenceNumber accepts minimum value 1 when input is "1"
214. EventVersion accepts minimum value 1 when input is "1"
215. AttemptNumber accepts minimum value 1 when input is "1"

### ParseError Display formatting (8 variants)

216. ParseError::Empty Display contains type_name
217. ParseError::InvalidCharacters Display contains type_name and invalid_chars
218. ParseError::InvalidFormat Display contains type_name and reason
219. ParseError::ExceedsMaxLength Display contains type_name, max, and actual
220. ParseError::BoundaryViolation Display contains type_name and reason
221. ParseError::NotAnInteger Display contains type_name and input
222. ParseError::ZeroValue Display contains type_name
223. ParseError::OutOfRange Display contains type_name, value, min, and max

### From<SequenceNumber> for NonZeroU64

224. From<SequenceNumber> for NonZeroU64 returns correct NonZeroU64 when converted

---

## 2. Trophy Allocation

| # | Behavior Category | Layer | Count | Justification |
|---|---|---|---|---|
| 1-54 | parse() constructors (happy + error) | Unit | 54 | Pure functions, deterministic, no I/O. Every variant needs exact assertion. |
| 55-68 | Accessor methods (as_str, as_u64) | Unit | 14 | Zero-copy borrows and value returns. Trivially pure. |
| 69-78 | new_unchecked (valid + panic) | Unit | 10 | Unsafe-by-convention constructor. Panic path needs `#[should_panic]`. |
| 79-90 | Conversion methods | Unit | 12 | Pure conversions to std lib types. `now()` is the only non-deterministic one (tests tolerance). |
| 91-104 | Display implementations | Unit | 14 | String formatting. Pure. |
| 105-118 | Round-trip (parse + Display) | Unit | 14 | Composition of two pure functions. |
| 119-168 | Serde (serialize + deserialize + reject) | Integration | 50 | serde is an external component. Tests real serde behavior, not mocks. |
| 169-178 | Integer/whitespace edge cases | Unit | 10 | Pure validation logic. |
| 179-215 | Additional boundary behaviors (promoted from matrix) | Unit | 37 | Pure validation logic — boundary values, whitespace, unicode, null bytes, u64::MAX, negative rejection. |
| 216-223 | ParseError Display formatting | Integration | 8 | Tests the thiserror-derived Display impl. External derive macro. |
| 224 | From<SequenceNumber> for NonZeroU64 | Unit | 1 | Pure conversion. |
| — | No Default compile-time gate | Static | 1 | Enforced via `#[cfg(doctest)]` or negative-compile test. |
| — | clippy + cargo-deny | Static | 2 | CI lint gates. |

**Totals:** 167 unit / 58 integration / 0 e2e / 3 static = 228 scenarios (209 distinct BDD + 26 proptest invariants; note: some integration scenarios are parameterized, inflating the function-level count)

---

## 3. BDD Scenarios

### 3.1 ParseError Display

#### Behavior: ParseError::Empty displays type_name in error message
```
Given: a ParseError::Empty { type_name: "InstanceId" }
When: Display::fmt is called
Then: output contains "InstanceId: value must not be empty"
```
Test: `fn parse_error_empty_displays_type_name_when_formatted()`

#### Behavior: ParseError::InvalidCharacters displays type_name and invalid_chars
```
Given: a ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: " @!".to_string() }
When: Display::fmt is called
Then: output contains "WorkflowName: invalid characters: \" @!\""
```
Test: `fn parse_error_invalid_characters_displays_details_when_formatted()`

#### Behavior: ParseError::InvalidFormat displays type_name and reason
```
Given: a ParseError::InvalidFormat { type_name: "InstanceId", reason: "expected 26 characters, got 5".to_string() }
When: Display::fmt is called
Then: output contains "InstanceId: invalid format: expected 26 characters, got 5"
```
Test: `fn parse_error_invalid_format_displays_reason_when_formatted()`

#### Behavior: ParseError::ExceedsMaxLength displays type_name, max, and actual
```
Given: a ParseError::ExceedsMaxLength { type_name: "WorkflowName", max: 128, actual: 200 }
When: Display::fmt is called
Then: output contains "WorkflowName: exceeds maximum length of 128 (got 200)"
```
Test: `fn parse_error_exceeds_max_length_displays_bounds_when_formatted()`

#### Behavior: ParseError::BoundaryViolation displays type_name and reason
```
Given: a ParseError::BoundaryViolation { type_name: "WorkflowName", reason: "must not start with hyphen".to_string() }
When: Display::fmt is called
Then: output contains "WorkflowName: must not start with hyphen"
```
Test: `fn parse_error_boundary_violation_displays_reason_when_formatted()`

#### Behavior: ParseError::NotAnInteger displays type_name and input
```
Given: a ParseError::NotAnInteger { type_name: "SequenceNumber", input: "abc".to_string() }
When: Display::fmt is called
Then: output contains "SequenceNumber: not a valid unsigned integer: abc"
```
Test: `fn parse_error_not_an_integer_displays_input_when_formatted()`

#### Behavior: ParseError::ZeroValue displays type_name
```
Given: a ParseError::ZeroValue { type_name: "SequenceNumber" }
When: Display::fmt is called
Then: output contains "SequenceNumber: value must not be zero"
```
Test: `fn parse_error_zero_value_displays_type_name_when_formatted()`

#### Behavior: ParseError::OutOfRange displays type_name, value, min, and max
```
Given: a ParseError::OutOfRange { type_name: "MaxAttempts", value: 0, min: 1, max: 100 }
When: Display::fmt is called
Then: output contains "MaxAttempts: value 0 is out of range (must be 1..=100)"
```
Test: `fn parse_error_out_of_range_displays_bounds_when_formatted()`

---

### 3.2 InstanceId

#### Behavior: InstanceId accepts valid ULID
```
Given: input "01H5JYV4XHGSR2F8KZ9BWNRFMA" (26-char Crockford Base32)
When: InstanceId::parse(input) is called
Then: returns Ok(InstanceId) where as_str() == "01H5JYV4XHGSR2F8KZ9BWNRFMA"
```
Test: `fn instance_id_accepts_valid_ulid_when_input_is_wellformed()`

#### Behavior: InstanceId rejects empty input
```
Given: input ""
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "InstanceId" })
```
Test: `fn instance_id_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: InstanceId rejects wrong length
```
Given: input "01H5JYV4XH" (10 chars)
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "26"
```
Test: `fn instance_id_rejects_wrong_length_with_invalid_format_when_input_is_not_26_chars()`

#### Behavior: InstanceId rejects invalid Crockford Base32 characters
```
Given: input "01H5JYV4XHGSR2F8KZ9BWNRFM@" (contains '@')
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "character" or "invalid"
```
Test: `fn instance_id_rejects_invalid_chars_with_invalid_format_when_input_has_non_crockford_chars()`

#### Behavior: InstanceId rejects malformed ULID (fails ulid crate validation)
```
Given: input "00000000000000000000000000" (26 chars but invalid timestamp/entropy)
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "ULID" or "validation"
```
Test: `fn instance_id_rejects_malformed_ulid_with_invalid_format_when_ulid_validation_fails()`

#### Behavior: InstanceId rejects wrong length (long)
```
Given: input "01H5JYV4XHGSR2F8KZ9BWNRFMAAAA" (29 chars)
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "26"
```
Test: `fn instance_id_rejects_long_input_with_invalid_format_when_input_exceeds_26_chars()`

#### Behavior: InstanceId rejects leading whitespace
```
Given: input " 01H5JYV4XHGSR2F8KZ9BWNRFMA"
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "26" or "character"
```
Test: `fn instance_id_rejects_leading_whitespace_with_invalid_format_when_input_has_space_prefix()`

#### Behavior: InstanceId rejects trailing whitespace
```
Given: input "01H5JYV4XHGSR2F8KZ9BWNRFMA "
When: InstanceId::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "InstanceId", reason: _ }) where reason contains "26" or "character"
```
Test: `fn instance_id_rejects_trailing_whitespace_with_invalid_format_when_input_has_space_suffix()`

---

### 3.3 WorkflowName

#### Behavior: WorkflowName accepts valid identifier
```
Given: input "deploy-production_v2"
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str() == "deploy-production_v2"
```
Test: `fn workflow_name_accepts_valid_identifier_when_chars_match_pattern()`

#### Behavior: WorkflowName rejects empty input
```
Given: input ""
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "WorkflowName" })
```
Test: `fn workflow_name_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: WorkflowName rejects invalid characters
```
Given: input "deploy job" (contains space)
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " })
```
Test: `fn workflow_name_rejects_invalid_chars_when_input_contains_space()`

#### Behavior: WorkflowName rejects input exceeding 128 characters
```
Given: input of 129 'a' characters
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::ExceedsMaxLength { type_name: "WorkflowName", max: 128, actual: 129 })
```
Test: `fn workflow_name_rejects_exceeds_max_length_when_input_is_129_chars()`

#### Behavior: WorkflowName accepts exactly 128 characters
```
Given: input of 128 'a' characters
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str().len() == 128
```
Test: `fn workflow_name_accepts_exactly_128_chars_when_at_boundary()`

#### Behavior: WorkflowName rejects leading hyphen
```
Given: input "-deploy"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "hyphen"
```
Test: `fn workflow_name_rejects_leading_hyphen_with_boundary_violation_when_starts_with_hyphen()`

#### Behavior: WorkflowName rejects leading underscore
```
Given: input "_deploy"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "underscore"
```
Test: `fn workflow_name_rejects_leading_underscore_with_boundary_violation_when_starts_with_underscore()`

#### Behavior: WorkflowName rejects trailing hyphen
```
Given: input "deploy-"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "hyphen"
```
Test: `fn workflow_name_rejects_trailing_hyphen_with_boundary_violation_when_ends_with_hyphen()`

#### Behavior: WorkflowName rejects trailing underscore
```
Given: input "deploy_"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "underscore"
```
Test: `fn workflow_name_rejects_trailing_underscore_with_boundary_violation_when_ends_with_underscore()`

#### Behavior: WorkflowName rejects hyphen-only input
```
Given: input "-"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "hyphen"
```
Test: `fn workflow_name_rejects_hyphen_only_with_boundary_violation_when_input_is_single_hyphen()`

#### Behavior: WorkflowName rejects underscore-only input
```
Given: input "_"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "WorkflowName", reason: _ }) where reason contains "underscore"
```
Test: `fn workflow_name_rejects_underscore_only_with_boundary_violation_when_input_is_single_underscore()`

#### Behavior: WorkflowName rejects leading whitespace (no stripping)
```
Given: input " deploy"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " })
```
Test: `fn workflow_name_rejects_leading_whitespace_with_invalid_chars_when_input_starts_with_space()`

#### Behavior: WorkflowName accepts single character
```
Given: input "a"
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str() == "a"
```
Test: `fn workflow_name_accepts_single_char_when_input_is_one_valid_character()`

#### Behavior: WorkflowName accepts valid with hyphen only
```
Given: input "deploy-production"
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str() == "deploy-production"
```
Test: `fn workflow_name_accepts_valid_with_hyphen_when_input_contains_hyphen()`

#### Behavior: WorkflowName accepts valid with underscore only
```
Given: input "deploy_production"
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str() == "deploy_production"
```
Test: `fn workflow_name_accepts_valid_with_underscore_when_input_contains_underscore()`

#### Behavior: WorkflowName accepts valid with digits
```
Given: input "v2-node"
When: WorkflowName::parse(input) is called
Then: returns Ok(WorkflowName) where as_str() == "v2-node"
```
Test: `fn workflow_name_accepts_valid_with_digits_when_input_contains_digits()`

#### Behavior: WorkflowName rejects trailing whitespace
```
Given: input "deploy "
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " })
```
Test: `fn workflow_name_rejects_trailing_whitespace_with_invalid_chars_when_input_ends_with_space()`

#### Behavior: WorkflowName rejects null byte
```
Given: input "deploy\x00"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: "\x00" })
```
Test: `fn workflow_name_rejects_null_byte_with_invalid_chars_when_input_contains_null()`

#### Behavior: WorkflowName rejects unicode combining character
```
Given: input "deploy-cafe\u{301}"
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: _ }) where invalid_chars is non-empty
```
Test: `fn workflow_name_rejects_unicode_combining_char_with_invalid_chars_when_input_has_composing_mark()`

#### Behavior: WorkflowName rejects whitespace-only input
```
Given: input " " (single space)
When: WorkflowName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " })
```
Test: `fn workflow_name_rejects_whitespace_only_with_invalid_chars_when_input_is_single_space()`

---

### 3.4 NodeName

#### Behavior: NodeName accepts valid identifier
```
Given: input "compile-artifact"
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str() == "compile-artifact"
```
Test: `fn node_name_accepts_valid_identifier_when_chars_match_pattern()`

#### Behavior: NodeName rejects empty input
```
Given: input ""
When: NodeName::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "NodeName" })
```
Test: `fn node_name_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: NodeName rejects invalid characters
```
Given: input "compile artifact" (contains space)
When: NodeName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "NodeName", invalid_chars: " " })
```
Test: `fn node_name_rejects_invalid_chars_when_input_contains_space()`

#### Behavior: NodeName rejects input exceeding 128 characters
```
Given: input of 129 'a' characters
When: NodeName::parse(input) is called
Then: returns Err(ParseError::ExceedsMaxLength { type_name: "NodeName", max: 128, actual: 129 })
```
Test: `fn node_name_rejects_exceeds_max_length_when_input_is_129_chars()`

#### Behavior: NodeName accepts exactly 128 characters
```
Given: input of 128 'a' characters
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str().len() == 128
```
Test: `fn node_name_accepts_exactly_128_chars_when_at_boundary()`

#### Behavior: NodeName rejects leading hyphen
```
Given: input "-compile"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "hyphen"
```
Test: `fn node_name_rejects_leading_hyphen_with_boundary_violation_when_starts_with_hyphen()`

#### Behavior: NodeName rejects leading underscore
```
Given: input "_compile"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "underscore"
```
Test: `fn node_name_rejects_leading_underscore_with_boundary_violation_when_starts_with_underscore()`

#### Behavior: NodeName rejects trailing hyphen
```
Given: input "compile-"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "hyphen"
```
Test: `fn node_name_rejects_trailing_hyphen_with_boundary_violation_when_ends_with_hyphen()`

#### Behavior: NodeName rejects trailing underscore
```
Given: input "compile_"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "underscore"
```
Test: `fn node_name_rejects_trailing_underscore_with_boundary_violation_when_ends_with_underscore()`

#### Behavior: NodeName rejects hyphen-only input
```
Given: input "-"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "hyphen"
```
Test: `fn node_name_rejects_hyphen_only_with_boundary_violation_when_input_is_single_hyphen()`

#### Behavior: NodeName rejects underscore-only input
```
Given: input "_"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::BoundaryViolation { type_name: "NodeName", reason: _ }) where reason contains "underscore"
```
Test: `fn node_name_rejects_underscore_only_with_boundary_violation_when_input_is_single_underscore()`

#### Behavior: NodeName rejects leading whitespace (no stripping)
```
Given: input " compile"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "NodeName", invalid_chars: " " })
```
Test: `fn node_name_rejects_leading_whitespace_with_invalid_chars_when_input_starts_with_space()`

#### Behavior: NodeName accepts single character
```
Given: input "a"
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str() == "a"
```
Test: `fn node_name_accepts_single_char_when_input_is_one_valid_character()`

#### Behavior: NodeName accepts valid with hyphen only
```
Given: input "compile-artifact"
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str() == "compile-artifact"
```
Test: `fn node_name_accepts_valid_with_hyphen_when_input_contains_hyphen()`

#### Behavior: NodeName accepts valid with underscore only
```
Given: input "compile_artifact"
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str() == "compile_artifact"
```
Test: `fn node_name_accepts_valid_with_underscore_when_input_contains_underscore()`

#### Behavior: NodeName accepts valid with digits
```
Given: input "node-42"
When: NodeName::parse(input) is called
Then: returns Ok(NodeName) where as_str() == "node-42"
```
Test: `fn node_name_accepts_valid_with_digits_when_input_contains_digits()`

#### Behavior: NodeName rejects trailing whitespace
```
Given: input "compile "
When: NodeName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "NodeName", invalid_chars: " " })
```
Test: `fn node_name_rejects_trailing_whitespace_with_invalid_chars_when_input_ends_with_space()`

#### Behavior: NodeName rejects null byte
```
Given: input "compile\x00"
When: NodeName::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "NodeName", invalid_chars: "\x00" })
```
Test: `fn node_name_rejects_null_byte_with_invalid_chars_when_input_contains_null()`

---

### 3.5 BinaryHash

#### Behavior: BinaryHash accepts valid lowercase hex
```
Given: input "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789" (64 chars)
When: BinaryHash::parse(input) is called
Then: returns Ok(BinaryHash) where as_str() == input
```
Test: `fn binary_hash_accepts_valid_lowercase_hex_when_input_is_wellformed()`

#### Behavior: BinaryHash accepts minimum 8-char hex
```
Given: input "abcdef01" (8 chars, even)
When: BinaryHash::parse(input) is called
Then: returns Ok(BinaryHash) where as_str() == "abcdef01"
```
Test: `fn binary_hash_accepts_8_char_hex_when_at_minimum_boundary()`

#### Behavior: BinaryHash rejects empty input
```
Given: input ""
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "BinaryHash" })
```
Test: `fn binary_hash_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: BinaryHash rejects uppercase hex characters
```
Given: input "ABCDEF0123456789" (uppercase)
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "BinaryHash", invalid_chars: "ABCDEF" })
```
Test: `fn binary_hash_rejects_uppercase_hex_with_invalid_chars_when_input_has_uppercase()`

#### Behavior: BinaryHash rejects non-hex characters
```
Given: input "ghijklmn"
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "BinaryHash", invalid_chars: "ghijklmn" })
```
Test: `fn binary_hash_rejects_non_hex_with_invalid_chars_when_input_has_non_hex()`

#### Behavior: BinaryHash rejects odd-length input
```
Given: input "abc" (3 chars, odd)
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "BinaryHash", reason: _ }) where reason contains "odd"
```
Test: `fn binary_hash_rejects_odd_length_with_invalid_format_when_length_is_odd()`

#### Behavior: BinaryHash rejects too-short input (less than 8)
```
Given: input "ab" (2 chars, even but < 8)
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "BinaryHash", reason: _ }) where reason contains "8"
```
Test: `fn binary_hash_rejects_too_short_with_invalid_format_when_length_is_less_than_8()`

#### Behavior: BinaryHash rejects 6-char input (even but too short)
```
Given: input "abcdef" (6 chars)
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidFormat { type_name: "BinaryHash", reason: _ }) where reason contains "8" or "minimum"
```
Test: `fn binary_hash_rejects_6_chars_with_invalid_format_when_below_minimum()`

#### Behavior: BinaryHash accepts valid longer hex (100 chars)
```
Given: input "a" repeated 100 times (even length, >= 8)
When: BinaryHash::parse(input) is called
Then: returns Ok(BinaryHash) where as_str().len() == 100
```
Test: `fn binary_hash_accepts_100_char_hex_when_within_valid_range()`

#### Behavior: BinaryHash rejects mixed case
```
Given: input "AbCdEf01" (contains uppercase A, C, E)
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "BinaryHash", invalid_chars: _ }) where invalid_chars contains at least one uppercase letter
```
Test: `fn binary_hash_rejects_mixed_case_with_invalid_chars_when_input_has_uppercase()`

#### Behavior: BinaryHash rejects whitespace prefix
```
Given: input " abcdef01"
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "BinaryHash", invalid_chars: " " })
```
Test: `fn binary_hash_rejects_leading_whitespace_with_invalid_chars_when_input_has_space_prefix()`

#### Behavior: BinaryHash rejects trailing whitespace
```
Given: input "abcdef01 "
When: BinaryHash::parse(input) is called
Then: returns Err(ParseError::InvalidCharacters { type_name: "BinaryHash", invalid_chars: " " })
```
Test: `fn binary_hash_rejects_trailing_whitespace_with_invalid_chars_when_input_has_space_suffix()`

#### Behavior: BinaryHash accepts all-zeros minimum
```
Given: input "00000000" (8 chars, even, all zeros)
When: BinaryHash::parse(input) is called
Then: returns Ok(BinaryHash) where as_str() == "00000000"
```
Test: `fn binary_hash_accepts_all_zeros_when_at_minimum_boundary()`

---

### 3.6 SequenceNumber

#### Behavior: SequenceNumber accepts valid nonzero decimal
```
Given: input "42"
When: SequenceNumber::parse(input) is called
Then: returns Ok(SequenceNumber) where as_u64() == 42
```
Test: `fn sequence_number_accepts_valid_nonzero_decimal_when_input_parses()`

#### Behavior: SequenceNumber accepts u64::MAX
```
Given: input "18446744073709551615"
When: SequenceNumber::parse(input) is called
Then: returns Ok(SequenceNumber) where as_u64() == 18446744073709551615
```
Test: `fn sequence_number_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: SequenceNumber rejects non-integer
```
Given: input "abc"
When: SequenceNumber::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "SequenceNumber", input: "abc" })
```
Test: `fn sequence_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: SequenceNumber rejects zero
```
Given: input "0"
When: SequenceNumber::parse(input) is called
Then: returns Err(ParseError::ZeroValue { type_name: "SequenceNumber" })
```
Test: `fn sequence_number_rejects_zero_with_zero_value_when_input_is_zero()`

#### Behavior: SequenceNumber accepts minimum value 1
```
Given: input "1"
When: SequenceNumber::parse(input) is called
Then: returns Ok(SequenceNumber) where as_u64() == 1
```
Test: `fn sequence_number_accepts_minimum_when_value_is_1()`

---

### 3.7 EventVersion

#### Behavior: EventVersion accepts valid nonzero decimal
```
Given: input "1"
When: EventVersion::parse(input) is called
Then: returns Ok(EventVersion) where as_u64() == 1
```
Test: `fn event_version_accepts_valid_nonzero_decimal_when_input_parses()`

#### Behavior: EventVersion accepts u64::MAX
```
Given: input "18446744073709551615"
When: EventVersion::parse(input) is called
Then: returns Ok(EventVersion) where as_u64() == 18446744073709551615
```
Test: `fn event_version_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: EventVersion rejects non-integer
```
Given: input "not-a-version"
When: EventVersion::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "EventVersion", input: "not-a-version" })
```
Test: `fn event_version_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: EventVersion rejects zero
```
Given: input "0"
When: EventVersion::parse(input) is called
Then: returns Err(ParseError::ZeroValue { type_name: "EventVersion" })
```
Test: `fn event_version_rejects_zero_with_zero_value_when_input_is_zero()`

#### Behavior: EventVersion accepts minimum value 1
```
Given: input "1"
When: EventVersion::parse(input) is called
Then: returns Ok(EventVersion) where as_u64() == 1
```
Test: `fn event_version_accepts_minimum_when_value_is_1()`

---

### 3.8 AttemptNumber

#### Behavior: AttemptNumber accepts valid nonzero decimal
```
Given: input "1"
When: AttemptNumber::parse(input) is called
Then: returns Ok(AttemptNumber) where as_u64() == 1
```
Test: `fn attempt_number_accepts_valid_nonzero_decimal_when_input_parses()`

#### Behavior: AttemptNumber accepts u64::MAX
```
Given: input "18446744073709551615"
When: AttemptNumber::parse(input) is called
Then: returns Ok(AttemptNumber) where as_u64() == 18446744073709551615
```
Test: `fn attempt_number_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: AttemptNumber rejects non-integer
```
Given: input "retry"
When: AttemptNumber::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "AttemptNumber", input: "retry" })
```
Test: `fn attempt_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: AttemptNumber rejects zero
```
Given: input "0"
When: AttemptNumber::parse(input) is called
Then: returns Err(ParseError::ZeroValue { type_name: "AttemptNumber" })
```
Test: `fn attempt_number_rejects_zero_with_zero_value_when_input_is_zero()`

#### Behavior: AttemptNumber accepts minimum value 1
```
Given: input "1"
When: AttemptNumber::parse(input) is called
Then: returns Ok(AttemptNumber) where as_u64() == 1
```
Test: `fn attempt_number_accepts_minimum_when_value_is_1()`

---

### 3.9 TimerId

#### Behavior: TimerId accepts non-empty string within limit
```
Given: input "timer-abc-123"
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str() == "timer-abc-123"
```
Test: `fn timer_id_accepts_non_empty_string_when_within_length_limit()`

#### Behavior: TimerId accepts string with any characters (opaque)
```
Given: input "timer@#$%^&*()"
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str() == "timer@#$%^&*()"
```
Test: `fn timer_id_accepts_any_non_empty_chars_when_opaque_string()`

#### Behavior: TimerId rejects empty input
```
Given: input ""
When: TimerId::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "TimerId" })
```
Test: `fn timer_id_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: TimerId rejects input exceeding 256 characters
```
Given: input of 257 'a' characters
When: TimerId::parse(input) is called
Then: returns Err(ParseError::ExceedsMaxLength { type_name: "TimerId", max: 256, actual: 257 })
```
Test: `fn timer_id_rejects_exceeds_max_length_when_input_is_257_chars()`

#### Behavior: TimerId accepts exactly 256 characters
```
Given: input of 256 'a' characters
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str().len() == 256
```
Test: `fn timer_id_accepts_exactly_256_chars_when_at_boundary()`

#### Behavior: TimerId accepts single character
```
Given: input "a"
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str() == "a"
```
Test: `fn timer_id_accepts_single_char_when_input_is_one_character()`

#### Behavior: TimerId accepts unicode
```
Given: input "\u{00e9}\u{00f1}" (éñ)
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str() == "\u{00e9}\u{00f1}"
```
Test: `fn timer_id_accepts_unicode_when_input_has_non_ascii_chars()`

#### Behavior: TimerId accepts trailing whitespace (opaque — preserves input)
```
Given: input "timer " (5 chars, trailing space)
When: TimerId::parse(input) is called
Then: returns Ok(TimerId) where as_str() == "timer "
```
Test: `fn timer_id_accepts_trailing_whitespace_when_opaque_type_preserves_input()`

---

### 3.10 IdempotencyKey

#### Behavior: IdempotencyKey accepts non-empty string within limit
```
Given: input "key-20240101-abc"
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str() == "key-20240101-abc"
```
Test: `fn idempotency_key_accepts_non_empty_string_when_within_length_limit()`

#### Behavior: IdempotencyKey accepts string with any characters (opaque)
```
Given: input "key@\t\n!()"
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str() == "key@\t\n!()"
```
Test: `fn idempotency_key_accepts_any_non_empty_chars_when_opaque_string()`

#### Behavior: IdempotencyKey rejects empty input
```
Given: input ""
When: IdempotencyKey::parse(input) is called
Then: returns Err(ParseError::Empty { type_name: "IdempotencyKey" })
```
Test: `fn idempotency_key_rejects_empty_with_empty_error_when_input_is_empty()`

#### Behavior: IdempotencyKey rejects input exceeding 1024 characters
```
Given: input of 1025 'b' characters
When: IdempotencyKey::parse(input) is called
Then: returns Err(ParseError::ExceedsMaxLength { type_name: "IdempotencyKey", max: 1024, actual: 1025 })
```
Test: `fn idempotency_key_rejects_exceeds_max_length_when_input_is_1025_chars()`

#### Behavior: IdempotencyKey accepts exactly 1024 characters
```
Given: input of 1024 'b' characters
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str().len() == 1024
```
Test: `fn idempotency_key_accepts_exactly_1024_chars_when_at_boundary()`

#### Behavior: IdempotencyKey accepts single character
```
Given: input "a"
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str() == "a"
```
Test: `fn idempotency_key_accepts_single_char_when_input_is_one_character()`

#### Behavior: IdempotencyKey accepts unicode
```
Given: input "key-\u{00e9}" (key-é)
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str() == "key-\u{00e9}"
```
Test: `fn idempotency_key_accepts_unicode_when_input_has_non_ascii_chars()`

#### Behavior: IdempotencyKey accepts trailing whitespace (opaque — preserves input)
```
Given: input "key " (4 chars, trailing space)
When: IdempotencyKey::parse(input) is called
Then: returns Ok(IdempotencyKey) where as_str() == "key "
```
Test: `fn idempotency_key_accepts_trailing_whitespace_when_opaque_type_preserves_input()`

---

### 3.11 TimeoutMs

#### Behavior: TimeoutMs accepts valid nonzero decimal
```
Given: input "5000"
When: TimeoutMs::parse(input) is called
Then: returns Ok(TimeoutMs) where as_u64() == 5000
```
Test: `fn timeout_ms_accepts_valid_nonzero_decimal_when_input_parses()`

#### Behavior: TimeoutMs accepts minimum value 1
```
Given: input "1"
When: TimeoutMs::parse(input) is called
Then: returns Ok(TimeoutMs) where as_u64() == 1
```
Test: `fn timeout_ms_accepts_minimum_when_value_is_1()`

#### Behavior: TimeoutMs rejects non-integer
```
Given: input "5s"
When: TimeoutMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "TimeoutMs", input: "5s" })
```
Test: `fn timeout_ms_rejects_non_integer_with_not_an_integer_when_input_is_duration_string()`

#### Behavior: TimeoutMs rejects zero
```
Given: input "0"
When: TimeoutMs::parse(input) is called
Then: returns Err(ParseError::ZeroValue { type_name: "TimeoutMs" })
```
Test: `fn timeout_ms_rejects_zero_with_zero_value_when_input_is_zero()`

#### Behavior: TimeoutMs::to_duration returns correct Duration
```
Given: TimeoutMs with inner value 5000
When: to_duration() is called
Then: returns Duration::from_millis(5000)
```
Test: `fn timeout_ms_to_duration_returns_correct_duration_when_called()`

#### Behavior: TimeoutMs accepts u64::MAX
```
Given: input "18446744073709551615"
When: TimeoutMs::parse(input) is called
Then: returns Ok(TimeoutMs) where as_u64() == 18446744073709551615
```
Test: `fn timeout_ms_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: TimeoutMs rejects negative
```
Given: input "-1"
When: TimeoutMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "TimeoutMs", input: "-1" })
```
Test: `fn timeout_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus()`

---

### 3.12 DurationMs

#### Behavior: DurationMs accepts valid decimal including zero
```
Given: input "0"
When: DurationMs::parse(input) is called
Then: returns Ok(DurationMs) where as_u64() == 0
```
Test: `fn duration_ms_accepts_zero_when_input_is_zero()`

#### Behavior: DurationMs accepts nonzero decimal
```
Given: input "1500"
When: DurationMs::parse(input) is called
Then: returns Ok(DurationMs) where as_u64() == 1500
```
Test: `fn duration_ms_accepts_nonzero_decimal_when_input_parses()`

#### Behavior: DurationMs accepts u64::MAX
```
Given: input "18446744073709551615"
When: DurationMs::parse(input) is called
Then: returns Ok(DurationMs) where as_u64() == 18446744073709551615
```
Test: `fn duration_ms_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: DurationMs rejects non-integer
```
Given: input "1.5s"
When: DurationMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "DurationMs", input: "1.5s" })
```
Test: `fn duration_ms_rejects_non_integer_with_not_an_integer_when_input_is_float_string()`

#### Behavior: DurationMs::to_duration returns correct Duration including zero
```
Given: DurationMs with inner value 0
When: to_duration() is called
Then: returns Duration::from_millis(0)
```
Test: `fn duration_ms_to_duration_returns_zero_duration_when_value_is_zero()`

#### Behavior: DurationMs::to_duration returns correct nonzero Duration
```
Given: DurationMs with inner value 2000
When: to_duration() is called
Then: returns Duration::from_millis(2000)
```
Test: `fn duration_ms_to_duration_returns_correct_duration_when_value_is_nonzero()`

#### Behavior: DurationMs rejects negative
```
Given: input "-1"
When: DurationMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "DurationMs", input: "-1" })
```
Test: `fn duration_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus()`

---

### 3.13 TimestampMs

#### Behavior: TimestampMs accepts valid decimal including zero
```
Given: input "0"
When: TimestampMs::parse(input) is called
Then: returns Ok(TimestampMs) where as_u64() == 0
```
Test: `fn timestamp_ms_accepts_zero_when_input_is_zero()`

#### Behavior: TimestampMs accepts nonzero decimal
```
Given: input "1710000000000"
When: TimestampMs::parse(input) is called
Then: returns Ok(TimestampMs) where as_u64() == 1710000000000
```
Test: `fn timestamp_ms_accepts_nonzero_decimal_when_input_parses()`

#### Behavior: TimestampMs rejects non-integer
```
Given: input "now"
When: TimestampMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "TimestampMs", input: "now" })
```
Test: `fn timestamp_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: TimestampMs::to_system_time returns UNIX_EPOCH when value is 0
```
Given: TimestampMs with inner value 0
When: to_system_time() is called
Then: returns std::time::SystemTime::UNIX_EPOCH
```
Test: `fn timestamp_ms_to_system_time_returns_unix_epoch_when_value_is_zero()`

#### Behavior: TimestampMs::to_system_time returns correct SystemTime when value > 0
```
Given: TimestampMs with inner value 1000
When: to_system_time() is called
Then: returns UNIX_EPOCH + Duration::from_millis(1000)
```
Test: `fn timestamp_ms_to_system_time_returns_correct_time_when_value_is_positive()`

#### Behavior: TimestampMs::now returns parseable value
```
Given: system clock is available
When: TimestampMs::now() is called
Then: TimestampMs::parse(&now.to_string()) returns Ok value where as_u64() == now.as_u64()
```
Test: `fn timestamp_ms_now_returns_parseable_value_when_system_clock_available()`

#### Behavior: TimestampMs::now is approximately current time
```
Given: system clock is available
When: TimestampMs::now() is called and stored as ts
Then: ts.as_u64() is within 5000ms of SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
```
Test: `fn timestamp_ms_now_is_approximately_current_time_when_called()`

#### Behavior: TimestampMs accepts u64::MAX
```
Given: input "18446744073709551615"
When: TimestampMs::parse(input) is called
Then: returns Ok(TimestampMs) where as_u64() == 18446744073709551615
```
Test: `fn timestamp_ms_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: TimestampMs rejects negative
```
Given: input "-1"
When: TimestampMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "TimestampMs", input: "-1" })
```
Test: `fn timestamp_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus()`

---

### 3.14 FireAtMs

#### Behavior: FireAtMs accepts valid decimal including zero
```
Given: input "0"
When: FireAtMs::parse(input) is called
Then: returns Ok(FireAtMs) where as_u64() == 0
```
Test: `fn fire_at_ms_accepts_zero_when_input_is_zero()`

#### Behavior: FireAtMs accepts nonzero decimal
```
Given: input "1710000000000"
When: FireAtMs::parse(input) is called
Then: returns Ok(FireAtMs) where as_u64() == 1710000000000
```
Test: `fn fire_at_ms_accepts_nonzero_decimal_when_input_parses()`

#### Behavior: FireAtMs rejects non-integer
```
Given: input "soon"
When: FireAtMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "FireAtMs", input: "soon" })
```
Test: `fn fire_at_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: FireAtMs::to_system_time returns correct SystemTime
```
Given: FireAtMs with inner value 5000
When: to_system_time() is called
Then: returns UNIX_EPOCH + Duration::from_millis(5000)
```
Test: `fn fire_at_ms_to_system_time_returns_correct_time_when_called()`

#### Behavior: FireAtMs::has_elapsed returns true when fire_at < now
```
Given: FireAtMs(1000) and TimestampMs(2000)
When: fire_at_ms.has_elapsed(timestamp_ms) is called
Then: returns true
```
Test: `fn fire_at_ms_has_elapsed_returns_true_when_fire_at_is_before_now()`

#### Behavior: FireAtMs::has_elapsed returns false when fire_at > now
```
Given: FireAtMs(3000) and TimestampMs(2000)
When: fire_at_ms.has_elapsed(timestamp_ms) is called
Then: returns false
```
Test: `fn fire_at_ms_has_elapsed_returns_false_when_fire_at_is_after_now()`

#### Behavior: FireAtMs::has_elapsed returns deterministic result when fire_at equals now
```
Given: FireAtMs(2000) and TimestampMs(2000)
When: fire_at_ms.has_elapsed(timestamp_ms) is called
Then: returns a consistent boolean (true or false — implementation choice, but MUST be deterministic)
```
Test: `fn fire_at_ms_has_elapsed_returns_deterministic_result_when_fire_at_equals_now()`

#### Behavior: FireAtMs accepts u64::MAX
```
Given: input "18446744073709551615"
When: FireAtMs::parse(input) is called
Then: returns Ok(FireAtMs) where as_u64() == 18446744073709551615
```
Test: `fn fire_at_ms_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: FireAtMs rejects negative
```
Given: input "-1"
When: FireAtMs::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "FireAtMs", input: "-1" })
```
Test: `fn fire_at_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus()`

---

### 3.15 MaxAttempts

#### Behavior: MaxAttempts accepts valid nonzero decimal
```
Given: input "3"
When: MaxAttempts::parse(input) is called
Then: returns Ok(MaxAttempts) where as_u64() == 3
```
Test: `fn max_attempts_accepts_valid_nonzero_decimal_when_input_parses()`

#### Behavior: MaxAttempts accepts minimum value 1
```
Given: input "1"
When: MaxAttempts::parse(input) is called
Then: returns Ok(MaxAttempts) where as_u64() == 1
```
Test: `fn max_attempts_accepts_minimum_when_value_is_1()`

#### Behavior: MaxAttempts rejects non-integer
```
Given: input "unlimited"
When: MaxAttempts::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "MaxAttempts", input: "unlimited" })
```
Test: `fn max_attempts_rejects_non_integer_with_not_an_integer_when_input_is_alpha()`

#### Behavior: MaxAttempts rejects zero
```
Given: input "0"
When: MaxAttempts::parse(input) is called
Then: returns Err(ParseError::ZeroValue { type_name: "MaxAttempts" })
```
Test: `fn max_attempts_rejects_zero_with_zero_value_when_input_is_zero()`

#### Behavior: MaxAttempts::is_exhausted returns false when attempt < max
```
Given: MaxAttempts(3) and AttemptNumber(1)
When: max_attempts.is_exhausted(attempt) is called
Then: returns false
```
Test: `fn max_attempts_is_exhausted_returns_false_when_attempt_less_than_max()`

#### Behavior: MaxAttempts::is_exhausted returns false when attempt is max minus one
```
Given: MaxAttempts(3) and AttemptNumber(2)
When: max_attempts.is_exhausted(attempt) is called
Then: returns false
```
Test: `fn max_attempts_is_exhausted_returns_false_when_attempt_is_max_minus_one()`

#### Behavior: MaxAttempts::is_exhausted returns true when attempt equals max
```
Given: MaxAttempts(3) and AttemptNumber(3)
When: max_attempts.is_exhausted(attempt) is called
Then: returns true
```
Test: `fn max_attempts_is_exhausted_returns_true_when_attempt_equals_max()`

#### Behavior: MaxAttempts::is_exhausted returns true when attempt exceeds max
```
Given: MaxAttempts(3) and AttemptNumber(5)
When: max_attempts.is_exhausted(attempt) is called
Then: returns true
```
Test: `fn max_attempts_is_exhausted_returns_true_when_attempt_exceeds_max()`

#### Behavior: MaxAttempts::is_exhausted returns true when max is 1 and attempt is 1
```
Given: MaxAttempts(1) and AttemptNumber(1)
When: max_attempts.is_exhausted(attempt) is called
Then: returns true
```
Test: `fn max_attempts_is_exhausted_returns_true_when_max_is_1_and_attempt_is_1()`

#### Behavior: MaxAttempts accepts u64::MAX
```
Given: input "18446744073709551615"
When: MaxAttempts::parse(input) is called
Then: returns Ok(MaxAttempts) where as_u64() == 18446744073709551615
```
Test: `fn max_attempts_accepts_u64_max_when_at_upper_boundary()`

#### Behavior: MaxAttempts rejects negative
```
Given: input "-1"
When: MaxAttempts::parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: "MaxAttempts", input: "-1" })
```
Test: `fn max_attempts_rejects_negative_with_not_an_integer_when_input_starts_with_minus()`

---

### 3.16 new_unchecked Constructors (5 NonZeroU64 types)

#### Behavior: SequenceNumber::new_unchecked constructs valid instance
```
Given: value 42u64
When: SequenceNumber::new_unchecked(42) is called
Then: returns SequenceNumber where as_u64() == 42
```
Test: `fn sequence_number_new_unchecked_constructs_when_value_is_nonzero()`

#### Behavior: SequenceNumber::new_unchecked panics on zero
```
Given: value 0u64
When: SequenceNumber::new_unchecked(0) is called
Then: thread panics
```
Test: `#[should_panic] fn sequence_number_new_unchecked_panics_when_value_is_zero()`

#### Behavior: EventVersion::new_unchecked constructs valid instance
```
Given: value 1u64
When: EventVersion::new_unchecked(1) is called
Then: returns EventVersion where as_u64() == 1
```
Test: `fn event_version_new_unchecked_constructs_when_value_is_nonzero()`

#### Behavior: EventVersion::new_unchecked panics on zero
```
Given: value 0u64
When: EventVersion::new_unchecked(0) is called
Then: thread panics
```
Test: `#[should_panic] fn event_version_new_unchecked_panics_when_value_is_zero()`

#### Behavior: AttemptNumber::new_unchecked constructs valid instance
```
Given: value 1u64
When: AttemptNumber::new_unchecked(1) is called
Then: returns AttemptNumber where as_u64() == 1
```
Test: `fn attempt_number_new_unchecked_constructs_when_value_is_nonzero()`

#### Behavior: AttemptNumber::new_unchecked panics on zero
```
Given: value 0u64
When: AttemptNumber::new_unchecked(0) is called
Then: thread panics
```
Test: `#[should_panic] fn attempt_number_new_unchecked_panics_when_value_is_zero()`

#### Behavior: TimeoutMs::new_unchecked constructs valid instance
```
Given: value 1000u64
When: TimeoutMs::new_unchecked(1000) is called
Then: returns TimeoutMs where as_u64() == 1000
```
Test: `fn timeout_ms_new_unchecked_constructs_when_value_is_nonzero()`

#### Behavior: TimeoutMs::new_unchecked panics on zero
```
Given: value 0u64
When: TimeoutMs::new_unchecked(0) is called
Then: thread panics
```
Test: `#[should_panic] fn timeout_ms_new_unchecked_panics_when_value_is_zero()`

#### Behavior: MaxAttempts::new_unchecked constructs valid instance
```
Given: value 3u64
When: MaxAttempts::new_unchecked(3) is called
Then: returns MaxAttempts where as_u64() == 3
```
Test: `fn max_attempts_new_unchecked_constructs_when_value_is_nonzero()`

#### Behavior: MaxAttempts::new_unchecked panics on zero
```
Given: value 0u64
When: MaxAttempts::new_unchecked(0) is called
Then: thread panics
```
Test: `#[should_panic] fn max_attempts_new_unchecked_panics_when_value_is_zero()`

---

### 3.17 Cross-cutting: Display Round-trip (14 newtypes)

#### Behavior: InstanceId Display round-trips through parse
```
Given: a valid InstanceId("01H5JYV4XHGSR2F8KZ9BWNRFMA")
When: format!("{}", id) produces string s, and InstanceId::parse(&s) is called
Then: returns Ok(value) where value == id
```
Test: `fn instance_id_display_round_trips_through_parse_when_valid()`

#### Behavior: WorkflowName Display round-trips through parse
```
Given: a valid WorkflowName("deploy-production")
When: format!("{}", wn) produces string s, and WorkflowName::parse(&s) is called
Then: returns Ok(value) where value == wn
```
Test: `fn workflow_name_display_round_trips_through_parse_when_valid()`

#### Behavior: NodeName Display round-trips through parse
```
Given: a valid NodeName("compile-artifact")
When: format!("{}", nn) produces string s, and NodeName::parse(&s) is called
Then: returns Ok(value) where value == nn
```
Test: `fn node_name_display_round_trips_through_parse_when_valid()`

#### Behavior: BinaryHash Display round-trips through parse
```
Given: a valid BinaryHash("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
When: format!("{}", bh) produces string s, and BinaryHash::parse(&s) is called
Then: returns Ok(value) where value == bh
```
Test: `fn binary_hash_display_round_trips_through_parse_when_valid()`

#### Behavior: SequenceNumber Display round-trips through parse
```
Given: a valid SequenceNumber(42)
When: format!("{}", sn) produces string s, and SequenceNumber::parse(&s) is called
Then: returns Ok(value) where value == sn
```
Test: `fn sequence_number_display_round_trips_through_parse_when_valid()`

#### Behavior: EventVersion Display round-trips through parse
```
Given: a valid EventVersion(1)
When: format!("{}", ev) produces string s, and EventVersion::parse(&s) is called
Then: returns Ok(value) where value == ev
```
Test: `fn event_version_display_round_trips_through_parse_when_valid()`

#### Behavior: AttemptNumber Display round-trips through parse
```
Given: a valid AttemptNumber(3)
When: format!("{}", an) produces string s, and AttemptNumber::parse(&s) is called
Then: returns Ok(value) where value == an
```
Test: `fn attempt_number_display_round_trips_through_parse_when_valid()`

#### Behavior: TimerId Display round-trips through parse
```
Given: a valid TimerId("timer-123")
When: format!("{}", ti) produces string s, and TimerId::parse(&s) is called
Then: returns Ok(value) where value == ti
```
Test: `fn timer_id_display_round_trips_through_parse_when_valid()`

#### Behavior: IdempotencyKey Display round-trips through parse
```
Given: a valid IdempotencyKey("key-abc")
When: format!("{}", ik) produces string s, and IdempotencyKey::parse(&s) is called
Then: returns Ok(value) where value == ik
```
Test: `fn idempotency_key_display_round_trips_through_parse_when_valid()`

#### Behavior: TimeoutMs Display round-trips through parse
```
Given: a valid TimeoutMs(5000)
When: format!("{}", tm) produces string s, and TimeoutMs::parse(&s) is called
Then: returns Ok(value) where value == tm
```
Test: `fn timeout_ms_display_round_trips_through_parse_when_valid()`

#### Behavior: DurationMs Display round-trips through parse
```
Given: a valid DurationMs(1500)
When: format!("{}", dm) produces string s, and DurationMs::parse(&s) is called
Then: returns Ok(value) where value == dm
```
Test: `fn duration_ms_display_round_trips_through_parse_when_valid()`

#### Behavior: TimestampMs Display round-trips through parse
```
Given: a valid TimestampMs(1710000000000)
When: format!("{}", ts) produces string s, and TimestampMs::parse(&s) is called
Then: returns Ok(value) where value == ts
```
Test: `fn timestamp_ms_display_round_trips_through_parse_when_valid()`

#### Behavior: FireAtMs Display round-trips through parse
```
Given: a valid FireAtMs(1710000000000)
When: format!("{}", fa) produces string s, and FireAtMs::parse(&s) is called
Then: returns Ok(value) where value == fa
```
Test: `fn fire_at_ms_display_round_trips_through_parse_when_valid()`

#### Behavior: MaxAttempts Display round-trips through parse
```
Given: a valid MaxAttempts(3)
When: format!("{}", ma) produces string s, and MaxAttempts::parse(&s) is called
Then: returns Ok(value) where value == ma
```
Test: `fn max_attempts_display_round_trips_through_parse_when_valid()`

---

### 3.18 Cross-cutting: Integer Edge Cases (shared across 8 integer newtypes)

These apply to: SequenceNumber, EventVersion, AttemptNumber, TimeoutMs, DurationMs, TimestampMs, FireAtMs, MaxAttempts.

#### Behavior: Integer newtypes accept leading zeros
```
Given: input "007" for any integer newtype
When: parse(input) is called
Then: returns Ok(value) where as_u64() == 7
```
Test: `fn integer_newtypes_accept_leading_zeros_when_input_has_prefix_zeros()`
(Use `rstest` parameterized test over all 8 integer types.)

#### Behavior: Integer newtypes reject hex prefix
```
Given: input "0xFF" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "0xFF" }) where exact_type_name is the name of the type under test (asserted per rstest case: "SequenceNumber", "EventVersion", "AttemptNumber", "TimeoutMs", "DurationMs", "TimestampMs", "FireAtMs", "MaxAttempts")
```
Test: `fn integer_newtypes_reject_hex_prefix_with_not_an_integer_when_input_starts_with_0x()`

#### Behavior: Integer newtypes reject octal prefix
```
Given: input "0o77" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "0o77" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_octal_prefix_with_not_an_integer_when_input_starts_with_0o()`

#### Behavior: Integer newtypes reject binary prefix
```
Given: input "0b101" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "0b101" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_binary_prefix_with_not_an_integer_when_input_starts_with_0b()`

#### Behavior: Integer newtypes reject negative sign
```
Given: input "-1" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "-1" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_negative_with_not_an_integer_when_input_starts_with_minus()`

#### Behavior: Integer newtypes reject overflow beyond u64::MAX
```
Given: input "18446744073709551616" (u64::MAX + 1) for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "18446744073709551616" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_overflow_with_not_an_integer_when_input_exceeds_u64_max()`

#### Behavior: Integer newtypes reject whitespace
```
Given: input " 42" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: " 42" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_leading_whitespace_with_not_an_integer_when_input_has_space_prefix()`

#### Behavior: Integer newtypes reject float notation
```
Given: input "3.14" for any integer newtype
When: parse(input) is called
Then: returns Err(ParseError::NotAnInteger { type_name: <exact_type_name>, input: "3.14" }) where exact_type_name is the name of the type under test (asserted per rstest case)
```
Test: `fn integer_newtypes_reject_float_notation_with_not_an_integer_when_input_has_decimal_point()`

---

### 3.19 Cross-cutting: Serde

#### Behavior: Serialize string newtype produces same string as Display
```
Given: a valid InstanceId("01H5JYV4XHGSR2F8KZ9BWNRFMA")
When: serde_json::to_string(&id) is called
Then: produces JSON string "\"01H5JYV4XHGSR2F8KZ9BWNRFMA\""
```
Test: `fn serde_serialize_string_newtype_matches_display_when_serialized()`
(Use `rstest` parameterized over all 6 string newtypes.)

#### Behavior: Serialize integer newtype produces same decimal as Display
```
Given: a valid SequenceNumber(42)
When: serde_json::to_string(&sn) is called
Then: produces JSON value "42"
```
Test: `fn serde_serialize_integer_newtype_matches_display_when_serialized()`
(Use `rstest` parameterized over all 8 integer newtypes.)

#### Behavior: Deserialize accepts valid string newtype input
```
Given: JSON string "\"01H5JYV4XHGSR2F8KZ9BWNRFMA\""
When: serde_json::from_str::<InstanceId>(json) is called
Then: returns Ok(InstanceId) where as_str() == "01H5JYV4XHGSR2F8KZ9BWNRFMA"
```
Test: `fn serde_deserialize_accepts_valid_string_newtype_when_json_is_valid()`
(Parameterized over all 6 string newtypes.)

#### Behavior: Deserialize accepts valid integer newtype input
```
Given: JSON value "42"
When: serde_json::from_str::<SequenceNumber>(json) is called
Then: returns Ok(SequenceNumber) where as_u64() == 42
```
Test: `fn serde_deserialize_accepts_valid_integer_newtype_when_json_is_valid()`
(Parameterized over all 8 integer newtypes.)

#### Behavior: Deserialize rejects invalid string newtype input
```
Given: JSON string "\"\"" (empty string)
When: serde_json::from_str::<InstanceId>(json) is called
Then: returns Err(_) where error message contains "InstanceId"
```
Test: `fn serde_deserialize_rejects_invalid_string_newtype_when_json_is_invalid()`
(Parameterized over all 6 string newtypes with type-appropriate invalid input.)

#### Behavior: Deserialize rejects invalid integer newtype input
```
Given: JSON value "abc" (not a valid integer)
When: serde_json::from_str::<SequenceNumber>(json) is called
Then: returns Err(_) where error message contains "SequenceNumber"
```
Test: `fn serde_deserialize_rejects_invalid_integer_newtype_when_json_is_non_integer()`
(Parameterized over all 8 integer newtypes.)

#### Behavior: Deserialize rejects zero for NonZeroU64 integer newtypes
```
Given: JSON value "0"
When: serde_json::from_str::<SequenceNumber>(json) is called
Then: returns Err(_) where error message contains "zero" or "SequenceNumber"
```
Test: `fn serde_deserialize_rejects_zero_for_nonzero_types_when_json_is_zero()`
(Parameterized over all 5 NonZeroU64 types: SequenceNumber, EventVersion, AttemptNumber, TimeoutMs, MaxAttempts.)

#### Behavior: Serde round-trip preserves value for string newtypes
```
Given: a valid WorkflowName("deploy-prod")
When: serialized to JSON and deserialized back
Then: result == Ok(original)
```
Test: `fn serde_round_trip_preserves_value_for_string_newtype_when_serialized_and_deserialized()`
(Parameterized over all 6 string newtypes.)

#### Behavior: Serde round-trip preserves value for integer newtypes
```
Given: a valid DurationMs(5000)
When: serialized to JSON and deserialized back
Then: result == Ok(original)
```
Test: `fn serde_round_trip_preserves_value_for_integer_newtype_when_serialized_and_deserialized()`
(Parameterized over all 8 integer newtypes.)

---

### 3.20 From<SequenceNumber> for NonZeroU64

#### Behavior: From<SequenceNumber> converts to NonZeroU64
```
Given: SequenceNumber with inner value 42
When: NonZeroU64::from(sn) is called
Then: returns NonZeroU64::new(42).unwrap()
```
Test: `fn from_sequence_number_returns_correct_nonzero_u64_when_converted()`

---

## 4. Proptest Invariants

### 4.1 Round-trip invariants (14 newtypes)

#### Proptest: InstanceId round-trip
```
Invariant: For any valid InstanceId v, parse(v.to_string()) == Ok(v)
Strategy: Generate valid 26-char ULID strings using ulid::Ulid::new().to_string()
Anti-invariant: Any string of length != 26 must fail
```

#### Proptest: WorkflowName round-trip
```
Invariant: For any valid WorkflowName v, parse(v.to_string()) == Ok(v)
Strategy: Non-empty strings of [a-zA-Z0-9_-], length 1..=128, first/last char NOT '-' or '_'
Anti-invariant: Empty string, length > 128, leading/trailing hyphen/underscore must fail
```

#### Proptest: NodeName round-trip
```
Invariant: For any valid NodeName v, parse(v.to_string()) == Ok(v)
Strategy: Same as WorkflowName (same constraints)
Anti-invariant: Same as WorkflowName
```

#### Proptest: BinaryHash round-trip
```
Invariant: For any valid BinaryHash v, parse(v.to_string()) == Ok(v)
Strategy: Even-length strings of [0-9a-f], length 8..=256
Anti-invariant: Odd-length, length < 8, uppercase chars, non-hex chars must fail
```

#### Proptest: SequenceNumber round-trip
```
Invariant: For any valid SequenceNumber v, parse(v.to_string()) == Ok(v)
Strategy: Non-zero u64 values (1..=u64::MAX)
Anti-invariant: "0" must fail with ZeroValue
```

#### Proptest: EventVersion round-trip
```
Invariant: For any valid EventVersion v, parse(v.to_string()) == Ok(v)
Strategy: Non-zero u64 values (1..=u64::MAX)
Anti-invariant: "0" must fail with ZeroValue
```

#### Proptest: AttemptNumber round-trip
```
Invariant: For any valid AttemptNumber v, parse(v.to_string()) == Ok(v)
Strategy: Non-zero u64 values (1..=u64::MAX)
Anti-invariant: "0" must fail with ZeroValue
```

#### Proptest: TimerId round-trip
```
Invariant: For any valid TimerId v, parse(v.to_string()) == Ok(v)
Strategy: Non-empty strings of any chars, length 1..=256
Anti-invariant: Empty string, length > 256 must fail
```

#### Proptest: IdempotencyKey round-trip
```
Invariant: For any valid IdempotencyKey v, parse(v.to_string()) == Ok(v)
Strategy: Non-empty strings of any chars, length 1..=1024
Anti-invariant: Empty string, length > 1024 must fail
```

#### Proptest: TimeoutMs round-trip
```
Invariant: For any valid TimeoutMs v, parse(v.to_string()) == Ok(v)
Strategy: Non-zero u64 values (1..=u64::MAX)
Anti-invariant: "0" must fail with ZeroValue
```

#### Proptest: DurationMs round-trip
```
Invariant: For any valid DurationMs v, parse(v.to_string()) == Ok(v)
Strategy: Any u64 value (0..=u64::MAX)
Anti-invariant: None — all valid u64 strings parse successfully
```

#### Proptest: TimestampMs round-trip
```
Invariant: For any valid TimestampMs v, parse(v.to_string()) == Ok(v)
Strategy: Any u64 value (0..=u64::MAX)
Anti-invariant: None — all valid u64 strings parse successfully
```

#### Proptest: FireAtMs round-trip
```
Invariant: For any valid FireAtMs v, parse(v.to_string()) == Ok(v)
Strategy: Any u64 value (0..=u64::MAX)
Anti-invariant: None — all valid u64 strings parse successfully
```

#### Proptest: MaxAttempts round-trip
```
Invariant: For any valid MaxAttempts v, parse(v.to_string()) == Ok(v)
Strategy: Non-zero u64 values (1..=u64::MAX)
Anti-invariant: "0" must fail with ZeroValue
```

---

### 4.2 Display consistency invariants

#### Proptest: String newtype Display is identity
```
Invariant: For any valid string newtype v, v.to_string() == v.as_str()
Strategy: Any valid input for each of the 6 string newtypes
```

#### Proptest: Integer newtype Display is decimal without padding
```
Invariant: For any valid integer newtype v, v.to_string() == v.as_u64().to_string()
Strategy: Any valid u64 value for each of the 8 integer newtypes
```

---

### 4.3 Hash/Eq consistency invariants

#### Proptest: Hash consistency for all newtypes
```
Invariant: If a == b then hash(a) == hash(b) for any two values of the same newtype
Strategy: Generate pairs of equal and unequal values for each newtype
Anti-invariant: If a != b, hash(a) MAY equal hash(b) (collision allowed, but unlikely)
```

#### Proptest: Clone equality for all newtypes
```
Invariant: For any valid value v, v.clone() == v
Strategy: Any valid input for each of the 14 newtypes
```

---

### 4.4 Copy trait invariant (integer newtypes only)

#### Proptest: Copy produces equal values
```
Invariant: For any valid integer newtype v, let copy = v; assert_eq!(copy, v)
Strategy: Any valid u64 value for each of the 8 integer newtypes
```

---

### 4.5 PartialOrd/Ord invariants (integer newtypes only)

#### Proptest: Ord is consistent with as_u64 comparison
```
Invariant: For any two integer values a, b of the same type: a.cmp(&b) == a.as_u64().cmp(&b.as_u64())
Strategy: Any two valid u64 values
```

---

### 4.6 Serde round-trip invariants

#### Proptest: Serde round-trip for all newtypes
```
Invariant: For any valid value v: serde_json::from_value(serde_json::to_value(v)?) == Ok(v)
Strategy: Any valid input for each of the 14 newtypes
```

---

### 4.7 Conversion method invariants

#### Proptest: TimeoutMs::to_duration round-trip
```
Invariant: For any valid TimeoutMs v: Duration::from_millis(v.as_u64()) == v.to_duration()
Strategy: Non-zero u64 values
```

#### Proptest: DurationMs::to_duration round-trip
```
Invariant: For any valid DurationMs v: Duration::from_millis(v.as_u64()) == v.to_duration()
Strategy: Any u64 value including 0
```

#### Proptest: TimestampMs::to_system_time consistency
```
Invariant: For any valid TimestampMs v: v.to_system_time() == SystemTime::UNIX_EPOCH + Duration::from_millis(v.as_u64())
Strategy: Any u64 value
```

#### Proptest: FireAtMs::has_elapsed consistency
```
Invariant: For any FireAtMs f and TimestampMs n: f.has_elapsed(n) == (f.as_u64() < n.as_u64()) [or <= depending on boundary decision]
Strategy: Any two u64 values
```

#### Proptest: MaxAttempts::is_exhausted consistency
```
Invariant: For any MaxAttempts m and AttemptNumber a: m.is_exhausted(a) == (a.as_u64() >= m.as_u64())
Strategy: Any two non-zero u64 values
```

---

## 5. Fuzz Targets

Every `parse()` method accepts untrusted `&str` input and is a fuzz boundary. Additionally, serde deserialization is a fuzz boundary.

### 5.1 parse() fuzz targets (14)

| # | Target Function | Input Type | Risk Class | Corpus Seeds |
|---|---|---|---|---|
| 1 | `InstanceId::parse` | `&[u8]` (as str) | Panic on malformed ULID, OOM on extremely long strings | `""`, `"01H5JYV4XHGSR2F8KZ9BWNRFMA"`, `"00000000000000000000000000"`, `"01H5JYV4XHGSR2F8KZ9BWNRFM@"`, `"\x00\x01\x02"`, `"AAAAAAAAAAAAAAAAAAAAAAAAAA"` (26 A's) |
| 2 | `WorkflowName::parse` | `&[u8]` (as str) | Panic on invalid UTF-8, logic error in boundary checks | `""`, `"a"`, `"-a"`, `"a-"`, `"_a"`, `"a_"`, `"a"*128`, `"a"*129`, `"a b"`, `"\t\n"`, `"a\x00b"` |
| 3 | `NodeName::parse` | `&[u8]` (as str) | Same as WorkflowName | Same as WorkflowName |
| 4 | `BinaryHash::parse` | `&[u8]` (as str) | Panic on odd-length check, logic error in min-length | `""`, `"ab"`, `"abcdef01"`, `"abcdef0"`, `"ABCDEF01"`, `"zzzz"`, `"0"*1000`, `"ff"*4` |
| 5 | `SequenceNumber::parse` | `&[u8]` (as str) | Panic on overflow, integer parsing edge case | `""`, `"0"`, `"1"`, `"18446744073709551615"`, `"18446744073709551616"`, `"-1"`, `"0xFF"`, `" 42"`, `"3.14"`, `"NaN"`, `"\x00"` |
| 6 | `EventVersion::parse` | `&[u8]` (as str) | Same as SequenceNumber | Same as SequenceNumber |
| 7 | `AttemptNumber::parse` | `&[u8]` (as str) | Same as SequenceNumber | Same as SequenceNumber |
| 8 | `TimerId::parse` | `&[u8]` (as str) | OOM on extremely long strings | `""`, `"a"`, `"a"*256`, `"a"*257`, `"\x00"`, `"\t\n\r"` |
| 9 | `IdempotencyKey::parse` | `&[u8]` (as str) | OOM on extremely long strings | `""`, `"a"`, `"a"*1024`, `"a"*1025`, `"\x00"` |
| 10 | `TimeoutMs::parse` | `&[u8]` (as str) | Same as SequenceNumber | Same as SequenceNumber |
| 11 | `DurationMs::parse` | `&[u8]` (as str) | Same as SequenceNumber but zero is valid | Same as SequenceNumber + `"0"` |
| 12 | `TimestampMs::parse` | `&[u8]` (as str) | Same as DurationMs | Same as DurationMs |
| 13 | `FireAtMs::parse` | `&[u8]` (as str) | Same as DurationMs | Same as DurationMs |
| 14 | `MaxAttempts::parse` | `&[u8]` (as str) | Same as SequenceNumber | Same as SequenceNumber |

### 5.2 Serde deserialization fuzz targets (14)

| # | Target Function | Input Type | Risk Class | Corpus Seeds |
|---|---|---|---|---|
| 1 | `serde_json::from_str::<InstanceId>` | `&[u8]` | Bypass of parse() validation via malformed JSON | Valid JSON strings for each type, invalid JSON, non-string JSON types |
| 2-14 | (same pattern for remaining 13 newtypes) | `&[u8]` | Same | Same pattern |

**Note:** Serde fuzz targets are lower priority than parse() fuzz targets because serde delegates to parse(). The primary risk is that a serde bug could bypass parse() validation. These should be implemented if parse() fuzzing reveals issues.

---

## 6. Kani Harnesses

### 6.1 Integer parse never panics
```
Property: For any byte sequence b: SequenceNumber::parse(core::str::from_utf8(&b)) does not panic
Bound: Input length <= 30 bytes
Rationale: PO-4 guarantees parse() never panics. Kani proves this for ALL inputs up to bound.
Applies to: All 8 integer newtypes.
```

### 6.2 NonZeroU64 inner value is always nonzero after successful parse
```
Property: For any input s where parse(s) == Ok(v): v.as_u64() != 0
Bound: Input length <= 30 bytes
Rationale: I-25/I-28/I-31/I-38/I-51 guarantee NonZeroU64 types never hold zero.
Kani proves the parse function cannot produce Ok with a zero inner value.
Applies to: SequenceNumber, EventVersion, AttemptNumber, TimeoutMs, MaxAttempts.
```

### 6.3 BinaryHash odd-length detection completeness
```
Property: For any input s with odd length > 0: BinaryHash::parse(s) != Ok(_)
Bound: Input length <= 256 bytes
Rationale: I-23 guarantees BinaryHash inner value always has even length.
Kani proves no odd-length string can produce Ok.
```

### 6.4 BinaryHash minimum length enforcement
```
Property: For any input s with even length < 8: BinaryHash::parse(s) != Ok(_)
Bound: Input length <= 16 bytes
Rationale: I-24 guarantees BinaryHash inner value is at least 8 characters.
Kani proves no short even-length string can produce Ok.
```

### 6.5 WorkflowName/NodeName boundary violation completeness
```
Property: For any input s starting or ending with '-' or '_': WorkflowName::parse(s) != Ok(_) and NodeName::parse(s) != Ok(_)
Bound: Input length <= 130 bytes
Rationale: I-16/I-20 guarantee no WorkflowName/NodeName starts or ends with hyphen or underscore.
Kani proves the boundary check catches ALL such inputs.
```

### 6.6 WorkflowName/NodeName ExceedsMaxLength enforcement
```
Property: For any input s with length > 128: WorkflowName::parse(s) != Ok(_) and NodeName::parse(s) != Ok(_)
Bound: Input length <= 130 bytes
Rationale: I-15/I-19 guarantee max 128 characters.
Kani proves no over-length input can produce Ok.
```

### 6.7 TimerId/IdempotencyKey length enforcement
```
Property: For any input s with length > 256: TimerId::parse(s) != Ok(_). For any input s with length > 1024: IdempotencyKey::parse(s) != Ok(_)
Bound: Input length <= 1030 bytes
Rationale: I-35/I-37 guarantee max lengths.
Kani proves no over-length input can produce Ok.
```

### 6.8 MaxAttempts::is_exhausted correctness
```
Property: For any MaxAttempts m and AttemptNumber a: m.is_exhausted(a) == (a.as_u64() >= m.as_u64())
Bound: Both values in 1..=u64::MAX
Rationale: This is a critical correctness property for retry logic. Wrong behavior means either premature failure or infinite retries.
Kani proves exhaustively for all u64 combinations within bound.
```

---

## 7. Mutation Testing Checkpoints

### Critical mutations to catch:

| Mutation | Which Test Catches It |
|---|---|
| Remove Empty check (early return for "") | `*_rejects_empty_*` for 6 string types (6 tests) |
| Swap Empty and InvalidCharacters priority | `workflow_name_rejects_leading_whitespace_*` (whitespace in 1-char string = InvalidCharacters, not Empty) |
| Remove ZeroValue check for NonZeroU64 types | `*_rejects_zero_with_zero_value_*` for 5 types (5 tests) |
| Swap ZeroValue and NotAnInteger priority | `sequence_number_rejects_zero_*` — "0" parses as u64 fine, must then check nonzero |
| Remove ExceedsMaxLength check | `*_rejects_exceeds_max_length_*` for 4 types (4 tests) |
| Remove BoundaryViolation check for WorkflowName | `workflow_name_rejects_leading_hyphen_*` (4 tests) |
| Remove BoundaryViolation check for NodeName | `node_name_rejects_leading_hyphen_*` (4 tests) |
| Allow uppercase in BinaryHash | `binary_hash_rejects_uppercase_hex_*` |
| Remove odd-length check for BinaryHash | `binary_hash_rejects_odd_length_*` |
| Remove min-length check for BinaryHash | `binary_hash_rejects_6_chars_*` |
| Change "0" from ZeroValue to NotAnInteger | `*_rejects_zero_*` must match ZeroValue variant, not NotAnInteger |
| Remove new_unchecked panic on zero | `#[should_panic] *_new_unchecked_panics_*` (5 tests) |
| to_duration multiplies by wrong factor | `*_to_duration_returns_correct_duration_*` |
| is_exhausted uses > instead of >= | `max_attempts_is_exhausted_returns_true_when_attempt_equals_max` |
| has_elapsed uses <= instead of < | `fire_at_ms_has_elapsed_returns_false_when_fire_at_is_after_now` |
| Display adds padding or prefix | `integer_newtype_display_is_decimal_without_padding` (proptest) |
| Serialize does not route through parse() | `serde_deserialize_rejects_invalid_*` (14 tests) |
| Deserialize bypasses validation | `serde_deserialize_rejects_invalid_*` (14 tests) |
| Change InstanceId length check from 26 to other | `instance_id_rejects_wrong_length_*` |

### Threshold

**Minimum mutation kill rate: 90%**

Rationale: With ~210 hand-written BDD scenarios plus 26 proptest invariants, the mutation kill rate should exceed 95%. Any surviving mutations indicate a missing test and must be addressed before merge.

### Mutation testing command

```bash
cargo mutants --workspace --exclude vo-common --exclude vo-core --exclude vo-storage \
  --exclude vo-actor --exclude vo-api --exclude vo-cli --exclude vo-frontend --exclude vo-linter
```

---

## 8. Combinatorial Coverage Matrix

### 8.1 InstanceId parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid ULID | `"01H5JYV4XHGSR2F8KZ9BWNRFMA"` | Ok(InstanceId) with as_str() == input | unit |
| empty | `""` | Err(Empty { type_name: "InstanceId" }) | unit |
| wrong length (short) | `"01H5JYV4XH"` | Err(InvalidFormat { type_name: "InstanceId", reason: contains "26" }) | unit |
| wrong length (long) | `"01H5JYV4XHGSR2F8KZ9BWNRFMAAAA"` (29 chars) | Err(InvalidFormat { type_name: "InstanceId", reason: contains "26" }) | unit |
| invalid chars | `"01H5JYV4XHGSR2F8KZ9BWNRFM@"` | Err(InvalidFormat { type_name: "InstanceId", reason: _ }) | unit |
| all zeros (26) | `"00000000000000000000000000"` | Err(InvalidFormat { type_name: "InstanceId", reason: _ }) | unit (ulid validation) |
| leading whitespace | `" 01H5JYV4XHGSR2F8KZ9BWNRFMA"` | Err(InvalidFormat { type_name: "InstanceId", reason: _ }) | unit |
| round-trip | any valid ULID | parse(display(v)) == Ok(v) | proptest |

### 8.2 WorkflowName parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid simple | `"deploy"` | Ok(WorkflowName) with as_str() == "deploy" | unit |
| valid with hyphen | `"deploy-production"` | Ok(WorkflowName) with as_str() == "deploy-production" | unit |
| valid with underscore | `"deploy_production"` | Ok(WorkflowName) with as_str() == "deploy_production" | unit |
| valid with digits | `"v2-node"` | Ok(WorkflowName) with as_str() == "v2-node" | unit |
| single valid char | `"a"` | Ok(WorkflowName) with as_str() == "a" | unit |
| at max length | `"a" * 128` | Ok(WorkflowName) with as_str().len() == 128 | unit |
| empty | `""` | Err(Empty { type_name: "WorkflowName" }) | unit |
| invalid char (space) | `"deploy job"` | Err(InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " }) | unit |
| invalid char (unicode) | `"deploy-cafe\u{301}"` | Err(InvalidCharacters { type_name: "WorkflowName", invalid_chars: _ }) | unit |
| invalid char (null) | `"deploy\x00"` | Err(InvalidCharacters { type_name: "WorkflowName", invalid_chars: "\x00" }) | unit |
| exceeds max length | `"a" * 129` | Err(ExceedsMaxLength { type_name: "WorkflowName", max: 128, actual: 129 }) | unit |
| leading hyphen | `"-deploy"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| leading underscore | `"_deploy"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| trailing hyphen | `"deploy-"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| trailing underscore | `"deploy_"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| hyphen only | `"-"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| underscore only | `"_"` | Err(BoundaryViolation { type_name: "WorkflowName", reason: _ }) | unit |
| leading whitespace | `" deploy"` | Err(InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " }) | unit |
| trailing whitespace | `"deploy "` | Err(InvalidCharacters { type_name: "WorkflowName", invalid_chars: " " }) | unit |
| round-trip | any valid input | parse(display(v)) == Ok(v) | proptest |

### 8.3 NodeName parse()

Identical structure to WorkflowName. Substitute `"NodeName"` for `type_name` in all error variants.

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid simple | `"compile"` | Ok(NodeName) | unit |
| all error cases | same patterns as WorkflowName | Same errors with type_name: "NodeName" | unit |
| round-trip | any valid input | parse(display(v)) == Ok(v) | proptest |

### 8.4 BinaryHash parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid SHA-256 | `"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"` (64 chars) | Ok(BinaryHash) | unit |
| valid min length | `"abcdef01"` (8 chars) | Ok(BinaryHash) | unit |
| valid longer | `"a" * 100` (100 chars, even) | Ok(BinaryHash) | unit |
| empty | `""` | Err(Empty { type_name: "BinaryHash" }) | unit |
| uppercase | `"ABCDEF01"` | Err(InvalidCharacters { type_name: "BinaryHash", invalid_chars: "ABCDEF" }) | unit |
| mixed case | `"AbCdEf01"` | Err(InvalidCharacters { type_name: "BinaryHash", invalid_chars: "ACE" }) | unit |
| non-hex | `"ghijklmn"` | Err(InvalidCharacters { type_name: "BinaryHash", invalid_chars: "ghijklmn" }) | unit |
| odd length (3) | `"abc"` | Err(InvalidFormat { type_name: "BinaryHash", reason: contains "odd" }) | unit |
| odd length (1) | `"a"` | Err(InvalidFormat { type_name: "BinaryHash", reason: contains "odd" }) | unit |
| too short even (2) | `"ab"` | Err(InvalidFormat { type_name: "BinaryHash", reason: contains "8" }) | unit |
| too short even (6) | `"abcdef"` | Err(InvalidFormat { type_name: "BinaryHash", reason: contains "8" }) | unit |
| at boundary (8) | `"00000000"` | Ok(BinaryHash) | unit |
| below boundary (8, odd) | `"0000000"` (7 chars) | Err(InvalidFormat — odd length checked first per priority) | unit |
| whitespace | `" abcdef01"` | Err(InvalidCharacters { type_name: "BinaryHash", invalid_chars: " " }) | unit |
| round-trip | any valid hex | parse(display(v)) == Ok(v) | proptest |

### 8.5 SequenceNumber parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| minimum valid | `"1"` | Ok(SequenceNumber) with as_u64() == 1 | unit |
| typical value | `"42"` | Ok(SequenceNumber) with as_u64() == 42 | unit |
| u64::MAX | `"18446744073709551615"` | Ok(SequenceNumber) with as_u64() == u64::MAX | unit |
| leading zeros | `"007"` | Ok(SequenceNumber) with as_u64() == 7 | unit |
| empty string | `""` | Err(NotAnInteger { type_name: "SequenceNumber", input: "" }) | unit |
| alpha | `"abc"` | Err(NotAnInteger { type_name: "SequenceNumber", input: "abc" }) | unit |
| zero | `"0"` | Err(ZeroValue { type_name: "SequenceNumber" }) | unit |
| negative | `"-1"` | Err(NotAnInteger { type_name: "SequenceNumber", input: "-1" }) | unit |
| hex prefix | `"0xFF"` | Err(NotAnInteger { type_name: "SequenceNumber", input: "0xFF" }) | unit |
| float | `"3.14"` | Err(NotAnInteger { type_name: "SequenceNumber", input: "3.14" }) | unit |
| overflow | `"18446744073709551616"` | Err(NotAnInteger { type_name: "SequenceNumber", input: "18446744073709551616" }) | unit |
| whitespace | `" 42"` | Err(NotAnInteger { type_name: "SequenceNumber", input: " 42" }) | unit |
| round-trip | any valid nonzero u64 string | parse(display(v)) == Ok(v) | proptest |

### 8.6 EventVersion parse()

Identical structure to SequenceNumber. Substitute `"EventVersion"` for `type_name`.

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| minimum valid | `"1"` | Ok(EventVersion) with as_u64() == 1 | unit |
| all error cases | same patterns as SequenceNumber | Same errors with type_name: "EventVersion" | unit |
| round-trip | any valid nonzero u64 string | parse(display(v)) == Ok(v) | proptest |

### 8.7 AttemptNumber parse()

Identical structure to SequenceNumber. Substitute `"AttemptNumber"` for `type_name`.

### 8.8 TimerId parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid simple | `"timer-123"` | Ok(TimerId) with as_str() == "timer-123" | unit |
| valid with special chars | `"timer@#$%^&*()"` | Ok(TimerId) with as_str() == "timer@#$%^&*()" | unit |
| valid with unicode | `"\u{00e9}\u{00f1}"` | Ok(TimerId) | unit |
| at max length | `"a" * 256` | Ok(TimerId) with as_str().len() == 256 | unit |
| empty | `""` | Err(Empty { type_name: "TimerId" }) | unit |
| exceeds max length | `"a" * 257` | Err(ExceedsMaxLength { type_name: "TimerId", max: 256, actual: 257 }) | unit |
| round-trip | any non-empty string <= 256 | parse(display(v)) == Ok(v) | proptest |

### 8.9 IdempotencyKey parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid simple | `"key-20240101"` | Ok(IdempotencyKey) | unit |
| valid with special chars | `"key@\t\n"` | Ok(IdempotencyKey) | unit |
| at max length | `"b" * 1024` | Ok(IdempotencyKey) with as_str().len() == 1024 | unit |
| empty | `""` | Err(Empty { type_name: "IdempotencyKey" }) | unit |
| exceeds max length | `"b" * 1025` | Err(ExceedsMaxLength { type_name: "IdempotencyKey", max: 1024, actual: 1025 }) | unit |
| round-trip | any non-empty string <= 1024 | parse(display(v)) == Ok(v) | proptest |

### 8.10 TimeoutMs parse()

Identical error structure to SequenceNumber. Substitute `"TimeoutMs"` for `type_name`.

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| minimum valid | `"1"` | Ok(TimeoutMs) with as_u64() == 1 | unit |
| typical value | `"5000"` | Ok(TimeoutMs) with as_u64() == 5000 | unit |
| all error cases | same patterns as SequenceNumber | Same errors with type_name: "TimeoutMs" | unit |
| round-trip | any valid nonzero u64 string | parse(display(v)) == Ok(v) | proptest |

### 8.11 DurationMs parse()

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| zero | `"0"` | Ok(DurationMs) with as_u64() == 0 | unit |
| nonzero | `"1500"` | Ok(DurationMs) with as_u64() == 1500 | unit |
| u64::MAX | `"18446744073709551615"` | Ok(DurationMs) with as_u64() == u64::MAX | unit |
| leading zeros | `"007"` | Ok(DurationMs) with as_u64() == 7 | unit |
| alpha | `"abc"` | Err(NotAnInteger { type_name: "DurationMs", input: "abc" }) | unit |
| empty | `""` | Err(NotAnInteger { type_name: "DurationMs", input: "" }) | unit |
| negative | `"-1"` | Err(NotAnInteger { type_name: "DurationMs", input: "-1" }) | unit |
| hex prefix | `"0xFF"` | Err(NotAnInteger { type_name: "DurationMs", input: "0xFF" }) | unit |
| round-trip | any valid u64 string | parse(display(v)) == Ok(v) | proptest |

### 8.12 TimestampMs parse()

Identical structure to DurationMs. Substitute `"TimestampMs"` for `type_name`.

### 8.13 FireAtMs parse()

Identical structure to DurationMs. Substitute `"FireAtMs"` for `type_name`.

### 8.14 MaxAttempts parse()

Identical error structure to SequenceNumber. Substitute `"MaxAttempts"` for `type_name`.

### 8.15 new_unchecked() (5 NonZeroU64 types)

| Scenario | Input Class | Expected Output | Layer |
|---|---|---|---|
| valid nonzero | `1u64` | Ok-equivalent (newtype with as_u64() == 1) | unit |
| valid large | `u64::MAX` | newtype with as_u64() == u64::MAX | unit |
| zero | `0u64` | panic (thread panics) | unit (`#[should_panic]`) |

### 8.16 Conversion methods

| Scenario | Input | Expected Output | Layer |
|---|---|---|---|
| TimeoutMs(1000).to_duration() | 1000ms | Duration::from_millis(1000) | unit |
| TimeoutMs(1).to_duration() | 1ms | Duration::from_millis(1) | unit |
| DurationMs(0).to_duration() | 0ms | Duration::from_millis(0) | unit |
| DurationMs(99999).to_duration() | 99999ms | Duration::from_millis(99999) | unit |
| TimestampMs(0).to_system_time() | 0ms | SystemTime::UNIX_EPOCH | unit |
| TimestampMs(1000).to_system_time() | 1000ms | UNIX_EPOCH + Duration::from_millis(1000) | unit |
| FireAtMs(5000).to_system_time() | 5000ms | UNIX_EPOCH + Duration::from_millis(5000) | unit |
| FireAtMs(1000).has_elapsed(TimestampMs(2000)) | fire_at < now | true | unit |
| FireAtMs(3000).has_elapsed(TimestampMs(2000)) | fire_at > now | false | unit |
| FireAtMs(2000).has_elapsed(TimestampMs(2000)) | fire_at == now | deterministic bool | unit |
| MaxAttempts(3).is_exhausted(AttemptNumber(1)) | attempt < max | false | unit |
| MaxAttempts(3).is_exhausted(AttemptNumber(2)) | attempt < max | false | unit |
| MaxAttempts(3).is_exhausted(AttemptNumber(3)) | attempt == max | true | unit |
| MaxAttempts(3).is_exhausted(AttemptNumber(5)) | attempt > max | true | unit |
| MaxAttempts(1).is_exhausted(AttemptNumber(1)) | attempt == max == 1 | true | unit |

### 8.17 ParseError variant coverage

| Variant | Newtype that produces it | Input that triggers it | Layer |
|---|---|---|---|
| Empty | InstanceId, WorkflowName, NodeName, BinaryHash, TimerId, IdempotencyKey | `""` | unit |
| InvalidCharacters | WorkflowName, NodeName, BinaryHash | Space, unicode, uppercase hex, non-hex | unit |
| InvalidFormat | InstanceId, BinaryHash | Wrong length, invalid ULID, odd hex length, too-short hex | unit |
| ExceedsMaxLength | WorkflowName (128), NodeName (128), TimerId (256), IdempotencyKey (1024) | String of max+1 chars | unit |
| BoundaryViolation | WorkflowName, NodeName | Leading/trailing hyphen or underscore | unit |
| NotAnInteger | All 8 integer types | "abc", "-1", "0xFF", "3.14" | unit |
| ZeroValue | SequenceNumber, EventVersion, AttemptNumber, TimeoutMs, MaxAttempts | "0" | unit |
| OutOfRange | None currently (reserved) | N/A | unit (variant existence) |

---

## Open Questions

1. **FireAtMs::has_elapsed equality semantics**: When `fire_at == now`, should `has_elapsed` return `true` or `false`? The contract says "check whether this fire-at time has elapsed" but does not specify the equality boundary. The test plan asserts determinism but does not prescribe a specific boolean. The implementer should document the choice.

2. **BinaryHash uppercase rejection detail**: When input contains both uppercase hex AND non-hex characters (e.g., `"GHIJ"`), should `InvalidCharacters.invalid_chars` contain all offending characters or only the first? The test plan checks that the error variant is `InvalidCharacters` with non-empty `invalid_chars` but does not prescribe exact character set collection order.

3. **InstanceId ULID validation granularity**: The `ulid` crate may reject strings for timestamp-out-of-range reasons vs character-set reasons. The contract maps both to `InvalidFormat`. Tests verify the variant but not the specific `reason` string for all ULID failures — only for the length check. This is acceptable per the contract's error taxonomy.

4. **Kani bounds**: The suggested bounds (input length <= 30 for integer types, <= 256 for string types) are practical limits for Kani's bounded model checking. If these prove too slow, bounds can be reduced without loss of confidence since the core validation logic is simple character/length checks.
