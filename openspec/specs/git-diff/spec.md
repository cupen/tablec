# git-diff Specification

## Purpose

Defines how the webui determines what changed in spreadsheet data by comparing the working tree against the current git branch's HEAD: repo resolution, per-cell diff status, per-file change status and counts, and the fallback behavior outside a git repo.

## Requirements

### Requirement: Repository resolution
The diff SHALL be computed between two sides: a left side that is either the working tree (`worktree`) or any resolvable git ref, and a right side that is any resolvable git ref, defaulting to `worktree` (left) and `HEAD` (right). The system SHALL locate the repository root by walking up from the working directory until a `.git` entry is found. If no repository is found, or the repository has no commits (no HEAD), the system SHALL enter a documented no-diff state rather than erroring. A ref that does not resolve to a commit SHALL be rejected with an error naming the ref.

#### Scenario: Working directory inside a repository
- **WHEN** the webui operates on a directory inside a git repository with at least one commit
- **THEN** each selected side resolves inside that repository; with defaults the right side baseline is the repository's current branch HEAD

#### Scenario: Branch-to-branch selection
- **WHEN** the left side is `feature-x` and the right side is `main`, both resolvable
- **THEN** the comparison is computed between those two refs' content

#### Scenario: No repository found
- **WHEN** the working directory is not inside any git repository
- **THEN** every spreadsheet reports no diff (clean) and the UI treats the file list as all-unchanged

#### Scenario: Repository with no commits
- **WHEN** the repository exists but has no HEAD commit
- **THEN** the system reports clean / no diff rather than failing

#### Scenario: Unresolvable ref rejected
- **WHEN** a side names a ref that git cannot resolve (e.g. `no-such-branch`)
- **THEN** the system rejects the request with an error naming that ref, never silently degrading to clean

### Requirement: Per-file git status
The compared file set SHALL be every supported spreadsheet that exists on either side and lies under the webui's resolved input directory. Each file SHALL be classified as the left side's change relative to the right side (the baseline): `added` (present only on the left), `deleted` (present only on the right), `modified` (present on both sides with differing content), or `clean`. When both sides are at their defaults (`worktree` vs `HEAD`), the system SHALL preserve the working-tree-specific statuses `untracked` (present in the working tree but not tracked at HEAD) and `added` (staged in the index) alongside `modified`, `deleted`, and `clean`. For `modified` files the system SHALL report counts of added and deleted lines derived from the git diff between the two sides.

#### Scenario: File present only on the left
- **WHEN** a spreadsheet exists at the left ref's tree but not at the right ref's tree
- **THEN** the file reports status `added`

#### Scenario: File present only on the right
- **WHEN** a spreadsheet exists at the right ref's tree but not at the left side
- **THEN** the file reports status `deleted`

#### Scenario: Modified spreadsheet
- **WHEN** a spreadsheet is tracked on both sides and its content differs
- **THEN** the file reports status `modified` with added/deleted counts

#### Scenario: Untracked spreadsheet
- **WHEN** both sides are at their defaults and a spreadsheet exists in the working tree but is not tracked by git
- **THEN** the file reports status `untracked` and is treated as changed

#### Scenario: Unchanged spreadsheet
- **WHEN** a spreadsheet is identical on both sides
- **THEN** the file reports status `clean` with zero change counts

### Requirement: Per-cell diff status in parsed preview
When the parsed preview is requested for a spreadsheet, each parsed cell of each data row SHALL carry a diff status computed between the left side's content and the right side's content: `added` (a new cell/row not present on the right side), `deleted` (a cell/row present on the right side but absent on the left side, or — for a file that exists only on the right side — every data row), `modified` (the cell exists in both but its value differs), or `unchanged`. Status derivation SHALL honor CSV/Excel cell equality: numeric values comparing equal across widths count as the same value.

#### Scenario: Changed cell value
- **WHEN** a cell's parsed value differs between the left and right sides
- **THEN** the cell reports status `modified`

#### Scenario: New row
- **WHEN** a row exists on the left side but not on the right side
- **THEN** every cell of that row reports status `added`

#### Scenario: Removed row
- **WHEN** a row exists on the right side but not on the left side
- **THEN** every cell of that row reports status `deleted`

#### Scenario: Deleted file previews as all-deleted
- **WHEN** the previewed spreadsheet exists only on the right side
- **THEN** every data row of the preview reports status `deleted`

#### Scenario: Numerically equal values
- **WHEN** a cell holds `1` (int) on one side and `1.0` (float) on the other
- **THEN** the cell reports status `unchanged`

### Requirement: Cell-level diff scoping SHALL be the previewed sheet
Per-cell diff SHALL apply only to the sheet being previewed. Sheets whose name starts with `#` are not parsed and therefore not diffed. Deleted sheets at HEAD (present at HEAD but missing from the workbook now) are out of scope for per-cell diff unless they are the previewed sheet.

#### Scenario: Diff applies to the previewed sheet
- **WHEN** the user previews one sheet of a workbook
- **THEN** per-cell statuses are computed for that sheet only

### Requirement: Diff requires parsed cell values
Per-cell diff statuses SHALL be computed over parsed cell values (what tablec reads), not raw cell text, so that `1` vs `1.0`, or a comment change in a header cell, do not produce spurious diffs.

#### Scenario: Whitespace-only change is not a value change
- **WHEN** a cell differs only by leading/trailing whitespace that normalizes to the same parsed value
- **THEN** the cell reports status `unchanged`

### Requirement: Failure and fallback behavior
If the git command fails for a file (corrupt repo, permission error, or missing binary), the system SHALL treat that file as clean/no-diff and attach a warning-level note rather than failing the whole file list. Tests for this capability SHALL skip when the `git` binary is unavailable.

#### Scenario: git binary missing
- **WHEN** `git` is not installed or a git command errors
- **THEN** the file list is still returned with every file reported clean and a non-fatal warning

#### Scenario: Deleted file
- **WHEN** a spreadsheet tracked at HEAD is missing from the working tree
- **THEN** the file reports status `deleted` and appears in the changed-files filter

### Requirement: Path containment and safety
The diff baseline SHALL be computed only for spreadsheet files under the webui's resolved input directory. Files outside the repository or outside the input directory MUST NOT be diffed against git. The resolution MUST NOT follow a directory symlink outside the repository root to diff an external file.

#### Scenario: File outside input directory
- **WHEN** a path resolves outside the webui's input directory
- **THEN** that path is not diffed and is reported clean

### Requirement: Modified-only filtering contract
The system SHALL expose a filter flag that selects only files with any change (status other than `clean`). The file-count badge reflects the active filter: total files when "All", changed files when "Modified only".

#### Scenario: Filter to changed files
- **WHEN** the modified-only filter is active
- **THEN** the left menu shows only files whose status is not `clean`

#### Scenario: Badge counts match the filter
- **WHEN** the filter is "All"
- **THEN** the badge shows the total number of files; when "Modified only", it shows the number of changed files

### Requirement: Direct content comparison
Comparisons SHALL compare the content at each side's ref tip directly — the semantics of `git diff A B`. The system SHALL NOT resolve or use a merge base for comparison.

#### Scenario: Diverged branches compare tips directly
- **WHEN** two branches have diverged from a common ancestor and the user compares them
- **THEN** the reported diff is the content difference between the two branch tips, not the changes either branch made since the merge base

### Requirement: Ref validation
Any ref naming a comparison side SHALL be resolved through git before any diff is computed. A ref that does not resolve SHALL produce a request-level error naming the ref; it MUST NOT silently degrade to clean/no-diff. The `worktree` keyword on the left side SHALL be accepted without git resolution.

#### Scenario: Unknown branch name
- **WHEN** a comparison is requested with the right side `no-such-branch`
- **THEN** the request fails with an error naming `no-such-branch`

#### Scenario: Tags and commit hashes are valid refs
- **WHEN** a side is named by a tag or an abbreviated commit hash that resolves
- **THEN** the comparison is computed against that commit