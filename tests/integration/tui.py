#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "textual>=0.47.0",
#     "httpx>=0.27.0",
#     "pysam>=0.22.0",
# ]
# ///
"""Interactive TUI for testing htsgetr server."""

import asyncio
import io
import json
import os
import subprocess
import signal
import sys
import tempfile
import time
from pathlib import Path

import httpx
import pysam
from textual import on
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Container, Horizontal, Vertical, VerticalScroll
from textual.widgets import (
    Button,
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    RichLog,
    Select,
    Static,
)


# Paths
PROJECT_ROOT = Path(__file__).parent.parent.parent
DATA_DIR = PROJECT_ROOT / "tests" / "data"
CARGO_MANIFEST = PROJECT_ROOT / "Cargo.toml"

# Server config
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8090


def get_test_files() -> list[dict]:
    """Get list of test data files with their types."""
    files = []
    if not DATA_DIR.exists():
        return files

    for path in sorted(DATA_DIR.iterdir()):
        if path.is_file():
            name = path.name
            size = path.stat().st_size

            # Determine file type and endpoint
            if name.endswith(".bam") and not name.endswith(".bai"):
                file_type = "BAM"
                endpoint = "reads"
                file_id = name.replace(".bam", "")
            elif name.endswith(".cram") and not name.endswith(".crai"):
                file_type = "CRAM"
                endpoint = "reads"
                file_id = name.replace(".cram", "")
            elif name.endswith(".vcf.gz") and not name.endswith(".tbi"):
                file_type = "VCF"
                endpoint = "variants"
                file_id = name.replace(".vcf.gz", "")
            elif name.endswith(".sam"):
                file_type = "SAM"
                endpoint = "reads"
                file_id = name.replace(".sam", "")
            else:
                # Index files or other
                continue

            # Use name as unique key since id can repeat (sample.bam, sample.cram, etc)
            files.append({
                "id": file_id,
                "name": name,
                "type": file_type,
                "endpoint": endpoint,
                "size": size,
                "path": str(path),
                "key": f"{file_id}.{file_type.lower()}",
            })

    return files


class ServerManager:
    """Manages the htsgetr server subprocess."""

    def __init__(self, host: str = DEFAULT_HOST, port: int = DEFAULT_PORT):
        self.host = host
        self.port = port
        self.process: asyncio.subprocess.Process | None = None
        self.base_url = f"http://{host}:{port}"
        self._reader_tasks: list[asyncio.Task] = []
        self._log_callback: callable | None = None

    def set_log_callback(self, callback: callable) -> None:
        """Set callback for logging server output."""
        self._log_callback = callback

    def _log(self, text: str) -> None:
        """Log text via callback if set."""
        if self._log_callback:
            self._log_callback(text)

    async def _read_stream(self, stream: asyncio.StreamReader, prefix: str = "") -> None:
        """Read from stream and log output."""
        while True:
            line = await stream.readline()
            if not line:
                break
            text = line.decode().rstrip()
            if text:
                self._log(f"{prefix}{text}")

    async def start(self) -> tuple[bool, str]:
        """Start the server. Returns (success, message)."""
        if self.process and self.process.returncode is None:
            return True, "Server already running"

        env = os.environ.copy()
        env["HTSGET_HOST"] = self.host
        env["HTSGET_PORT"] = str(self.port)
        env["HTSGET_DATA_DIR"] = str(DATA_DIR)
        env["RUST_LOG"] = "info"

        try:
            self.process = await asyncio.create_subprocess_exec(
                "cargo", "run", "--manifest-path", str(CARGO_MANIFEST),
                env=env,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=PROJECT_ROOT,
            )

            # Start background tasks to read stdout/stderr
            if self.process.stdout:
                task = asyncio.create_task(self._read_stream(self.process.stdout))
                self._reader_tasks.append(task)
            if self.process.stderr:
                task = asyncio.create_task(self._read_stream(self.process.stderr))
                self._reader_tasks.append(task)

            # Wait for server to be ready
            async with httpx.AsyncClient(timeout=1.0) as client:
                for _ in range(30):
                    await asyncio.sleep(0.2)
                    try:
                        resp = await client.get(f"{self.base_url}/service-info")
                        if resp.status_code == 200:
                            return True, f"Server started at {self.base_url}"
                    except httpx.RequestError:
                        pass
                    # Check if process died
                    if self.process.returncode is not None:
                        return False, "Server failed to start (check log above)"

            return False, "Server startup timed out"
        except Exception as e:
            return False, f"Failed to start server: {e}"

    async def stop(self) -> tuple[bool, str]:
        """Stop the server. Returns (success, message)."""
        if not self.process:
            return True, "No server to stop"

        if self.process.returncode is not None:
            self.process = None
            return True, "Server already stopped"

        try:
            self.process.terminate()
            await asyncio.wait_for(self.process.wait(), timeout=5)
            # Cancel reader tasks
            for task in self._reader_tasks:
                task.cancel()
            self._reader_tasks.clear()
            self.process = None
            return True, "Server stopped"
        except asyncio.TimeoutError:
            self.process.kill()
            for task in self._reader_tasks:
                task.cancel()
            self._reader_tasks.clear()
            self.process = None
            return True, "Server killed"
        except Exception as e:
            return False, f"Failed to stop server: {e}"

    def is_running(self) -> bool:
        """Check if server is running."""
        return self.process is not None and self.process.returncode is None




class HtsgetTUI(App):
    """Main TUI application for testing htsgetr."""

    CSS = """
    Screen {
        layout: grid;
        grid-size: 2 1;
        grid-columns: 1fr 2fr;
    }

    #left-panel {
        height: 100%;
        border: solid $primary;
        padding: 1;
    }

    #right-panel {
        height: 100%;
    }

    .section-title {
        text-style: bold;
        color: $text;
        margin-bottom: 1;
    }

    .form-row {
        height: 3;
        margin-bottom: 1;
    }

    .form-label {
        width: 12;
        padding-top: 1;
    }

    .button-row {
        height: 3;
        margin-top: 1;
    }

    #server-status {
        height: 3;
        margin-bottom: 1;
        padding: 1;
        background: $surface;
    }

    #server-status.running {
        background: $success 30%;
    }

    #server-status.stopped {
        background: $error 30%;
    }

    #files-table {
        height: 12;
        margin-bottom: 1;
    }

    #query-panel {
        height: auto;
        border: solid $secondary;
        padding: 1;
        margin-top: 1;
        margin-bottom: 1;
    }

    #urls-table {
        height: 8;
    }

    #request-response-panel {
        height: 2fr;
    }

    #request-panel, #response-panel {
        width: 1fr;
        border: solid $primary;
        padding: 1;
    }

    #bottom-panel {
        height: 1fr;
    }

    #server-panel, #url-content-panel {
        width: 1fr;
        border: solid $secondary;
        padding: 1;
    }

    .panel-title {
        text-style: bold;
        color: $text;
        margin-bottom: 1;
    }

    #request-log, #response-log, #server-log, #url-content-log {
        height: 1fr;
    }

    Button {
        margin-right: 1;
    }

    #execute-btn {
        margin-right: 1;
    }
    """

    BINDINGS = [
        Binding("q", "quit", "Quit"),
        Binding("s", "toggle_server", "Start/Stop Server"),
        Binding("r", "refresh_files", "Refresh Files"),
        Binding("enter", "execute_query", "Execute Query"),
    ]

    def __init__(self):
        super().__init__()
        self.server = ServerManager()
        self.test_files = get_test_files()
        self.http_client = httpx.AsyncClient(timeout=30.0)
        self.ticket_urls: list[dict] = []
        self.ticket_format: str = ""  # BAM, CRAM, VCF, etc.

    def compose(self) -> ComposeResult:
        yield Header()
        with Container(id="left-panel"):
            yield Static("Server: [red]Stopped[/red]", id="server-status", classes="stopped")
            with Horizontal(classes="button-row"):
                yield Button("Start Server", id="start-btn", variant="success")
                yield Button("Stop Server", id="stop-btn", variant="error")
            yield Label("Test Files", classes="section-title")
            yield DataTable(id="files-table")
            with Vertical(id="query-panel"):
                yield Label("Query Parameters", classes="section-title")
                with Horizontal(classes="form-row"):
                    yield Label("File ID:", classes="form-label")
                    yield Select([], id="file-select", allow_blank=True, prompt="Select file...")
                with Horizontal(classes="form-row"):
                    yield Label("Endpoint:", classes="form-label")
                    yield Select(
                        [("reads", "reads"), ("variants", "variants")],
                        id="endpoint-select",
                        value="reads",
                    )
                with Horizontal(classes="form-row"):
                    yield Label("Reference:", classes="form-label")
                    yield Input(placeholder="e.g., chr1 or 1", id="reference-input")
                with Horizontal(classes="form-row"):
                    yield Label("Start:", classes="form-label")
                    yield Input(placeholder="0", id="start-input", type="integer")
                with Horizontal(classes="form-row"):
                    yield Label("End:", classes="form-label")
                    yield Input(placeholder="1000000", id="end-input", type="integer")
                with Horizontal(classes="button-row"):
                    yield Button("Execute Query", id="execute-btn", variant="primary")
                    yield Button("Clear", id="clear-btn")
            yield Label("Ticket URLs", classes="section-title")
            yield DataTable(id="urls-table")
            yield Button("Fetch & Decode All", id="decode-btn", variant="warning")
        with Vertical(id="right-panel"):
            with Horizontal(id="request-response-panel"):
                with Vertical(id="request-panel"):
                    yield Label("Request", classes="panel-title")
                    yield RichLog(id="request-log", markup=True)
                with Vertical(id="response-panel"):
                    yield Label("Response", classes="panel-title")
                    yield RichLog(id="response-log", markup=True)
            with Horizontal(id="bottom-panel"):
                with Vertical(id="server-panel"):
                    yield Label("Server Log", classes="panel-title")
                    yield RichLog(id="server-log", markup=True)
                with Vertical(id="url-content-panel"):
                    yield Label("URL Content", classes="panel-title")
                    yield RichLog(id="url-content-log", markup=True)
        yield Footer()

    def on_mount(self) -> None:
        """Initialize the UI on mount."""
        self._populate_files_table()
        self._populate_file_select()
        self._init_urls_table()
        # Set up server log callback
        self.server.set_log_callback(self._log_server)

    def _init_urls_table(self) -> None:
        """Initialize the URLs table."""
        table = self.query_one("#urls-table", DataTable)
        table.add_columns("#", "Class", "URL")
        table.cursor_type = "row"

    def _populate_urls_table(self, urls: list[dict]) -> None:
        """Populate URLs table from ticket response."""
        self.ticket_urls = urls
        table = self.query_one("#urls-table", DataTable)
        table.clear()
        for i, url_info in enumerate(urls):
            url = url_info.get("url", "")
            url_class = url_info.get("class", "body")
            # Truncate long URLs for display
            display_url = url if len(url) < 50 else url[:47] + "..."
            table.add_row(str(i + 1), url_class, display_url, key=str(i))

    def _populate_files_table(self) -> None:
        """Populate the files data table."""
        table = self.query_one("#files-table", DataTable)
        table.clear(columns=True)
        table.add_columns("ID", "Type", "Endpoint", "Size")
        for f in self.test_files:
            size_str = f"{f['size']:,} B" if f['size'] < 1024 else f"{f['size'] // 1024:,} KB"
            table.add_row(f["id"], f["type"], f["endpoint"], size_str, key=f["key"])
        table.cursor_type = "row"

    def _populate_file_select(self) -> None:
        """Populate the file select dropdown."""
        select = self.query_one("#file-select", Select)
        # Show "id (TYPE)" as label, use key as value
        options = [(f"{f['id']} ({f['type']})", f["key"]) for f in self.test_files]
        select.set_options(options)
        if options:
            select.value = options[0][1]

    def _get_file_by_key(self, key: str) -> dict | None:
        """Look up file info by key."""
        for f in self.test_files:
            if f["key"] == key:
                return f
        return None

    def _update_server_status(self) -> None:
        """Update server status display."""
        status = self.query_one("#server-status", Static)
        if self.server.is_running():
            status.update(f"Server: [green]Running[/green] at {self.server.base_url}")
            status.remove_class("stopped")
            status.add_class("running")
        else:
            status.update("Server: [red]Stopped[/red]")
            status.remove_class("running")
            status.add_class("stopped")

    def _log_request(self, text: str) -> None:
        """Log to request panel."""
        log = self.query_one("#request-log", RichLog)
        log.write(text)

    def _log_response(self, text: str) -> None:
        """Log to response panel."""
        log = self.query_one("#response-log", RichLog)
        log.write(text)

    def _log_server(self, text: str) -> None:
        """Log to server panel."""
        log = self.query_one("#server-log", RichLog)
        log.write(text)

    @on(Button.Pressed, "#start-btn")
    async def handle_start_server(self) -> None:
        """Start the server."""
        self._log_server("Starting server...")
        success, msg = await self.server.start()
        self._log_server(msg)
        self._update_server_status()

    @on(Button.Pressed, "#stop-btn")
    async def handle_stop_server(self) -> None:
        """Stop the server."""
        self._log_server("Stopping server...")
        success, msg = await self.server.stop()
        self._log_server(msg)
        self._update_server_status()

    @on(Button.Pressed, "#clear-btn")
    def handle_clear(self) -> None:
        """Clear query inputs."""
        self.query_one("#reference-input", Input).value = ""
        self.query_one("#start-input", Input).value = ""
        self.query_one("#end-input", Input).value = ""

    @on(Button.Pressed, "#execute-btn")
    async def handle_execute(self) -> None:
        """Execute the query."""
        await self._execute_query()

    @on(Button.Pressed, "#decode-btn")
    async def handle_decode_all(self) -> None:
        """Fetch all ticket URLs, concatenate, and decode with pysam."""
        if not self.ticket_urls:
            self._log_response("[red]No ticket URLs to decode[/red]")
            return

        content_log = self.query_one("#url-content-log", RichLog)
        content_log.clear()
        content_log.write(f"[bold]Fetching {len(self.ticket_urls)} blocks...[/bold]")

        # Fetch all blocks and concatenate
        blocks = []
        for i, url_info in enumerate(self.ticket_urls):
            url = url_info.get("url", "")
            try:
                resp = await self.http_client.get(url)
                blocks.append(resp.content)
                content_log.write(f"  Block {i+1}: {len(resp.content)} bytes")
            except httpx.RequestError as e:
                content_log.write(f"[red]  Block {i+1} failed: {e}[/red]")
                return

        # Concatenate all blocks
        data = b"".join(blocks)
        content_log.write(f"\n[bold]Total: {len(data)} bytes[/bold]")
        content_log.write(f"Format: {self.ticket_format}\n")

        # Decode based on format
        try:
            if self.ticket_format in ("BAM", "CRAM"):
                # Write to temp file and read with pysam
                suffix = ".bam" if self.ticket_format == "BAM" else ".cram"
                with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as f:
                    f.write(data)
                    tmp_path = f.name

                try:
                    with pysam.AlignmentFile(tmp_path, "rb") as bam:
                        content_log.write(f"[green]Successfully decoded {self.ticket_format}[/green]")
                        content_log.write(f"References: {bam.references[:10]}{'...' if len(bam.references) > 10 else ''}")
                        content_log.write(f"Mapped: {bam.mapped}, Unmapped: {bam.unmapped}\n")

                        # Show first few reads
                        content_log.write("[bold]First reads:[/bold]")
                        for i, read in enumerate(bam.fetch(until_eof=True)):
                            if i >= 10:
                                content_log.write(f"  ... and more")
                                break
                            content_log.write(f"  {read.query_name}: {read.reference_name}:{read.reference_start}-{read.reference_end}")
                finally:
                    os.unlink(tmp_path)

            elif self.ticket_format == "VCF":
                # VCF is text, just display it
                text = data.decode("utf-8", errors="replace")
                content_log.write("[green]VCF content:[/green]")
                content_log.write(text[:5000])

            elif self.ticket_format == "BCF":
                # Write to temp file and read with pysam
                with tempfile.NamedTemporaryFile(suffix=".bcf", delete=False) as f:
                    f.write(data)
                    tmp_path = f.name

                try:
                    with pysam.VariantFile(tmp_path, "rb") as vcf:
                        content_log.write(f"[green]Successfully decoded BCF[/green]")
                        content_log.write(f"Samples: {list(vcf.header.samples)}")
                        content_log.write(f"\n[bold]First variants:[/bold]")
                        for i, rec in enumerate(vcf):
                            if i >= 10:
                                content_log.write(f"  ... and more")
                                break
                            content_log.write(f"  {rec.chrom}:{rec.pos} {rec.ref} -> {','.join(str(a) for a in rec.alts or [])}")
                finally:
                    os.unlink(tmp_path)
            else:
                content_log.write(f"[yellow]Unknown format: {self.ticket_format}, showing raw:[/yellow]")
                content_log.write(data[:2000].decode("utf-8", errors="replace"))

        except Exception as e:
            content_log.write(f"[red]Decode error: {e}[/red]")

    @on(DataTable.RowSelected, "#files-table")
    def handle_file_selected(self, event: DataTable.RowSelected) -> None:
        """Handle file selection in table."""
        if event.row_key:
            file_key = str(event.row_key.value)
            file_info = self._get_file_by_key(file_key)
            if file_info:
                # Update the select and endpoint based on selected file
                select = self.query_one("#file-select", Select)
                select.value = file_key
                endpoint_select = self.query_one("#endpoint-select", Select)
                endpoint_select.value = file_info["endpoint"]

    @on(DataTable.RowSelected, "#urls-table")
    async def handle_url_selected(self, event: DataTable.RowSelected) -> None:
        """Handle URL selection - fetch and display content."""
        if not event.row_key:
            return
        idx = int(event.row_key.value)
        if idx >= len(self.ticket_urls):
            return

        url_info = self.ticket_urls[idx]
        url = url_info.get("url", "")
        url_class = url_info.get("class", "body")

        content_log = self.query_one("#url-content-log", RichLog)
        content_log.clear()
        content_log.write(f"[bold]Fetching: {url}[/bold]")
        content_log.write(f"Class: {url_class}\n")

        try:
            resp = await self.http_client.get(url)
            content_log.write(f"Status: {resp.status_code}")
            content_log.write(f"Content-Type: {resp.headers.get('content-type', 'unknown')}")
            content_log.write(f"Content-Length: {len(resp.content)} bytes\n")

            # Detect text vs binary content
            content_type = resp.headers.get("content-type", "")
            # VCF, FASTA, FASTQ are text-based; BAM, CRAM, BCF are binary
            text_types = ("json", "text", "vcf", "fasta", "fastq")
            is_text = any(t in content_type.lower() for t in text_types)

            if is_text:
                content_log.write(resp.text[:4000])
            else:
                # Show hex dump for binary (BAM, CRAM, BCF)
                data = resp.content[:512]
                hex_lines = []
                for i in range(0, len(data), 16):
                    chunk = data[i:i+16]
                    hex_part = " ".join(f"{b:02x}" for b in chunk)
                    ascii_part = "".join(chr(b) if 32 <= b < 127 else "." for b in chunk)
                    hex_lines.append(f"{i:04x}  {hex_part:<48}  {ascii_part}")
                content_log.write("[dim]" + "\n".join(hex_lines) + "[/dim]")
        except httpx.RequestError as e:
            content_log.write(f"[red]Error: {e}[/red]")

    async def _execute_query(self) -> None:
        """Execute the htsget query."""
        if not self.server.is_running():
            self._log_response("[red]Error: Server is not running[/red]")
            return

        file_select = self.query_one("#file-select", Select)
        endpoint_select = self.query_one("#endpoint-select", Select)
        reference_input = self.query_one("#reference-input", Input)
        start_input = self.query_one("#start-input", Input)
        end_input = self.query_one("#end-input", Input)

        file_key = file_select.value
        if file_key == Select.BLANK:
            self._log_response("[red]Error: No file selected[/red]")
            return
        file_info = self._get_file_by_key(file_key)
        if not file_info:
            self._log_response("[red]Error: File not found[/red]")
            return
        file_id = file_info["id"]
        endpoint = endpoint_select.value
        reference = reference_input.value.strip() or None
        start = int(start_input.value) if start_input.value else None
        end = int(end_input.value) if end_input.value else None

        # Build URL
        url = f"{self.server.base_url}/{endpoint}/{file_id}"
        params = {}
        # Include format for non-default types (CRAM, BCF)
        if file_info["type"] in ("CRAM", "BCF"):
            params["format"] = file_info["type"]
        if reference:
            params["referenceName"] = reference
        if start is not None:
            params["start"] = start
        if end is not None:
            params["end"] = end

        # Log request
        self._log_request(f"{'='*40}")
        self._log_request(f"[bold]GET {url}[/bold]")
        if params:
            self._log_request(f"Params: {json.dumps(params, indent=2)}")

        # Execute and log response
        try:
            resp = await self.http_client.get(url, params=params)
            self._log_response(f"{'='*40}")
            self._log_response(f"Status: {resp.status_code}")

            try:
                data = resp.json()
                formatted = json.dumps(data, indent=2)
                self._log_response(formatted)

                if resp.status_code == 200 and "htsget" in data:
                    urls = data["htsget"].get("urls", [])
                    self.ticket_format = data["htsget"].get("format", "")
                    self._log_response(f"\n[dim]Ticket contains {len(urls)} URL(s), format: {self.ticket_format}[/dim]")
                    self._populate_urls_table(urls)
            except json.JSONDecodeError:
                self._log_response(resp.text[:1000])
        except httpx.RequestError as e:
            self._log_response(f"{'='*40}")
            self._log_response(f"[red]Request error: {e}[/red]")

    async def action_toggle_server(self) -> None:
        """Toggle server state."""
        if self.server.is_running():
            await self.handle_stop_server()
        else:
            await self.handle_start_server()

    def action_refresh_files(self) -> None:
        """Refresh the file list."""
        self.test_files = get_test_files()
        self._populate_files_table()
        self._populate_file_select()

    async def action_execute_query(self) -> None:
        """Execute query via keybinding."""
        await self._execute_query()

    async def action_quit(self) -> None:
        """Quit the app, stopping server first."""
        await self.server.stop()
        await self.http_client.aclose()
        self.exit()


def main():
    app = HtsgetTUI()
    app.run()


if __name__ == "__main__":
    main()
