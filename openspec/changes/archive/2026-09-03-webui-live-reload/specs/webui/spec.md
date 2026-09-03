## ADDED Requirements

### Requirement: Live file-change notifications
The webui server SHALL watch the resolved input directory for file-system changes and SHALL notify connected browsers over a WebSocket endpoint so the file list refreshes without a manual reload. Change detection SHALL cover creation, modification, removal, and rename of files anywhere under the input directory, including spreadsheets and configuration files. A detected change SHALL cause any connected client to re-fetch the file list; the server SHALL NOT start a build or check, and SHALL NOT itself re-run the schema parser, as a consequence of a change notification. Watching SHALL operate regardless of whether the directory is inside a git repository.

#### Scenario: Spreadsheet modified while viewing
- **WHEN** a spreadsheet under the input directory is modified while a browser client is connected
- **THEN** the client receives a change notification over the WebSocket and the refreshed file list reflects the modification

#### Scenario: New spreadsheet appears
- **WHEN** a spreadsheet is created under the input directory while a client is connected
- **THEN** the client receives a change notification and the refreshed file list includes the new file

#### Scenario: Spreadsheet removed
- **WHEN** a spreadsheet is deleted from the input directory while a client is connected
- **THEN** the client receives a change notification and the refreshed file list no longer includes the file

#### Scenario: Change does not trigger a build
- **WHEN** a change notification is delivered to a client
- **THEN** no build or check is started by the server as a side effect

### Requirement: WebSocket endpoint and client lifecycle
The server SHALL expose a WebSocket endpoint for change notifications. A connected client SHALL receive a notification message whenever the watcher detects a change in the input directory; the message SHALL indicate that the file list changed. If the connection drops, the client SHALL reconnect with a backoff delay and SHALL re-fetch the file list after a successful reconnect, so no inotify/ReadDirectoryChangesW events are lost to a disconnected browser. A client that cannot maintain a connection SHALL still function via the existing manual reload control.

#### Scenario: Client receives change message
- **WHEN** a client is connected to the WebSocket endpoint and a change occurs in the input directory
- **THEN** the client receives a message indicating the file list changed

#### Scenario: Reconnect after disconnect
- **WHEN** a client's WebSocket connection drops and the client reconnects
- **THEN** the client re-fetches the file list after the reconnect succeeds

#### Scenario: Manual reload remains available
- **WHEN** the WebSocket is unavailable or disconnected
- **THEN** the existing reload control still refreshes the file list