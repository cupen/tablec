## Purpose

Defines how the webui determines what changed in spreadsheet data by comparing the working tree against the current git branch's HEAD: repo resolution, per-cell diff status, per-file change status and counts, and the fallback behavior outside a git repo.

## ADDED Requirements

### Requirement: Repository resolution
The diff baseline SHALL be the current branch's HEAD commit of the git repository that contains the webui's working directory. The system SHALL locate the repository root by walking up from the working directory until a `.git` entry is found. If no repository is found, or the repository has no commits (no HEAD), the system SHALL enter a documented no-diff state rather than erroring.

#### Scenario: Working directory inside a repository
- **WHEN** the webui operates on a directory inside a git repository with at least one commit
- **THEN** the diff baseline is that repository's current branch HEAD

#### Scenario: No repository found
- **WHEN** the working directory is not inside any git repository
- **THEN** every spreadsheet reports no diff (clean) and the UI treats the file list as all-unchanged

#### Scenario: Repository with no commits
- **WHEN** the repository exists but has no HEAD commit
- **THEN** the system reports clean / no diff rather than failing

### Requirement: Per-file git status
Each spreadsheet file SHALL be classified as one of: `modified` (tracked and differs from HEAD), `added` (staged in the index), `untracked` (not tracked), `deleted` (tracked at HEAD but missing in the working tree), or `clean` (no changes). For `modified` files the system SHALL report counts of added and deleted lines (insertions/deletions from `git diff --numstat`).

#### Scenario: Untracked spreadsheet
- **WHEN** a spreadsheet exists in the working tree but is not tracked by git
- **THEN** the file reports status `untracked` and is treated as changed

#### Scenario: Modified spreadsheet
- **WHEN** a spreadsheet is tracked and differs from HEAD
- **THEN** the file reports status `modified` with added/deleted counts

#### Scenario: Unchanged spreadsheet
- **WHEN** a spreadsheet is tracked and identical to HEAD
- **THEN** the file reports status `clean` with zero change counts

### Requirement: Per-cell diff status in parsed preview
When the parsed preview is requested for a spreadsheet, each parsed cell of each data row SHALL carry a diff status relative to HEAD: `added` (a new cell/row not present at HEAD), `deleted` (a cell/row present at HEAD but absent now), `modified` (the cell exists in both but its value differs), or `unchanged`. Status derivation SHALL honor CSV/Excel cell equality: numeric values comparing equal across widths count as the same value.

#### Scenario: Changed cell value
- **WHEN** a cell's raw value differs between the working tree and HEAD
- **THEN** the cell reports status `modified`

#### Scenario: New row
- **WHEN** a row exists in the working tree but not at HEAD
- **THEN** every cell of that row reports status `added`

#### Scenario: Removed row
- **WHEN** a row exists at HEAD but not in the working tree
- **THEN** every cell of that row reports status `deleted`

#### Scenario: Numerically equal values
- **WHEN** a cell holds `1` (int) at HEAD and `1.0` (float) in the working tree
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