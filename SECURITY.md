# Security Audit Report

## Executive Summary

This document provides a comprehensive security audit of the `mcp-tui-driver` project, addressing concerns about code execution capabilities and network access.

**Last Updated:** 2026-02-11

## Overview

`mcp-tui-driver` is an MCP (Model Context Protocol) server that provides TUI (Terminal User Interface) automation capabilities. It allows LLMs to drive terminal applications through a controlled API.

## Scope of Audit

The audit examined:
1. Code execution capabilities beyond TUI driving
2. Network access and data transmission
3. File system access patterns
4. Dependency security
5. Input validation and sanitization

## Key Findings

### ✅ SAFE: No Network Communication

**Finding:** The application does NOT make any network requests or send data to external servers.

**Evidence:**
- Dependency analysis shows no HTTP client libraries (reqwest, hyper, ureq, curl, etc.)
- Code search reveals no TCP/UDP socket usage
- The only "network" reference is the `net` feature in tokio, which is used for local stdio communication
- Transport mechanisms are limited to:
  - **stdio (default):** stdin/stdout communication for local MCP integration
  - **SSE (not yet implemented):** Would be for local server mode only

**Transport Code (mcp-server/src/main.rs:47-50):**
```rust
// Use stdio transport
debug!("MCP TUI Driver starting with stdio transport");
let transport = rmcp::transport::stdio();
let service = server.serve(transport).await?;
```

### ⚠️ CONTROLLED: Code Execution via JavaScript Runtime

**Finding:** The application provides JavaScript code execution capabilities through the Boa engine, but this is intentional and controlled.

**Purpose:** JavaScript execution via `tui_run_code` tool allows for complex TUI automation workflows.

**Security Controls:**

1. **Sandboxed Environment:**
   - Uses Boa engine (pure Rust JavaScript implementation)
   - No access to Node.js APIs, filesystem APIs, or network APIs
   - Single-threaded, synchronous execution

2. **Limited API Surface:**
   JavaScript code can ONLY:
   - Read terminal screen content (`tui.text()`, `tui.snapshot()`)
   - Send keyboard input (`tui.sendText()`, `tui.pressKey()`)
   - Perform mouse actions (`tui.click()`, `tui.clickAt()`)
   - Control the terminal session (`tui.resize()`, `tui.sendSignal()`)
   - Take screenshots and save to `/tmp/tui-screenshots/<session-id>/` with filename sanitization
   - Access debug buffers (input/output logs)

3. **No Dangerous APIs:**
   The JavaScript environment does NOT provide:
   - File system access (except controlled screenshot saving)
   - Network access
   - Process spawning
   - System calls
   - Access to environment variables
   - Import/require mechanisms

4. **Path Traversal Protection (mcp-server/src/boa.rs:692-695):**
   ```rust
   // Sanitize: reject filenames with path separators or parent directory references
   if name.contains('/') || name.contains('\\') || name.contains("..") {
       return Err(/* error */);
   }
   ```

### ✅ CONTROLLED: Command Execution for TUI Applications

**Finding:** The application can execute arbitrary system commands, but this is the core purpose and is user-controlled.

**Purpose:** The `tui_launch` tool spawns TUI applications that users want to automate (e.g., htop, vim, tmux).

**Security Considerations:**

1. **User Intent:** Commands are explicitly provided by the user/LLM through the `tui_launch` tool
2. **PTY Isolation:** Processes run in pseudo-terminals (PTY) with controlled I/O
3. **No Shell Injection:** Uses `CommandBuilder` with proper argument separation
4. **Environment Control:** 
   - Inherits parent environment by default
   - Allows custom environment variables via `env` parameter
   - Forces `TERM=xterm-256color` for consistency

**Launch Code (tui-driver/src/driver.rs:232-242):**
```rust
// Build command
let mut cmd = CommandBuilder::new(&options.command);
for arg in &options.args {
    cmd.arg(arg);  // Arguments are properly separated
}
cmd.env("TERM", "xterm-256color");
for (key, value) in &options.env {
    cmd.env(key, value);
}
if let Some(cwd) = &options.cwd {
    cmd.cwd(cwd);
}
```

### ✅ CONTROLLED: File System Access

**Finding:** Limited file system access for recording and screenshots.

**Access Patterns:**

1. **Screenshot Storage (mcp-server/src/boa.rs:707-736):**
   - Writes to `/tmp/tui-screenshots/<session-id>/`
   - Filename sanitization prevents path traversal
   - PNG format only
   - Session-scoped directories

2. **Session Recording (tui-driver/src/recording.rs):**
   - Writes to user-specified path (from `tui_launch` recording options)
   - Asciicast v3 format (.cast files)
   - Records terminal I/O events only
   - User controls output path

3. **No File Reading:**
   - Application does NOT read arbitrary files
   - Only reads from PTY (terminal output)

### Dependency Security

**Core Dependencies:**
- `boa_engine` (0.21) - Pure Rust JavaScript engine
- `portable-pty` (0.8) - Cross-platform PTY management
- `wezterm-term` - Terminal emulation from WezTerm project
- `rmcp` (0.12) - MCP protocol implementation
- `tokio` (1.48) - Async runtime

**Network-Related Dependencies:**
- `tokio` includes networking features but they are NOT used for external communication
- No HTTP client libraries present
- No TLS/SSL libraries for external connections

**Recommendation:** Run `cargo audit` regularly to check for known vulnerabilities in dependencies.

## Attack Surface Analysis

### 1. MCP Protocol (JSON-RPC over stdio)

**Attack Vector:** Malformed or malicious MCP messages

**Mitigation:**
- Structured message parsing via `rmcp` library
- Type-safe Rust implementations
- Input validation on tool parameters

### 2. JavaScript Code Execution

**Attack Vector:** Malicious JavaScript in `tui_run_code`

**Risk Level:** LOW to MEDIUM

**Mitigation:**
- Sandboxed execution (no dangerous APIs)
- No filesystem access beyond controlled screenshot saving
- No network access
- Iteration limit prevents infinite loops (100k iterations)
- Timeout protection via Promise settling

**Residual Risk:** 
- Resource exhaustion through CPU-intensive operations
- Large memory allocations

**Recommendation:** Consider adding:
- Memory limits for JavaScript execution
- CPU time limits
- Maximum script length

### 3. Command Execution

**Attack Vector:** Malicious commands via `tui_launch`

**Risk Level:** MEDIUM (by design)

**Note:** This is an intended feature. Users are responsible for validating commands before launching.

**Mitigation:**
- No shell expansion (arguments properly separated)
- Runs with user's privileges (no elevation)
- PTY isolation

### 4. File System Access

**Attack Vector:** Path traversal in screenshot filenames

**Risk Level:** LOW

**Mitigation:**
- Explicit path traversal checks
- Restricted to `/tmp/tui-screenshots/<session-id>/`
- Filename sanitization

### 5. Session Recording

**Attack Vector:** Writing to arbitrary paths

**Risk Level:** LOW to MEDIUM

**Note:** User controls the output path, so they can specify any writable location.

**Mitigation:**
- User explicitly provides the path
- File write requires appropriate permissions
- No path traversal on read

## Recommendations

### High Priority

1. **Document Security Model:**
   - ✅ This document serves that purpose
   - Clearly state that users are responsible for validating commands

2. **Add Resource Limits for JavaScript:**
   ```rust
   // Consider adding:
   - Maximum memory allocation
   - CPU time limits
   - Maximum script size
   ```

### Medium Priority

3. **Path Validation for Recording:**
   - Consider restricting recording output to specific directories
   - Or require explicit user confirmation for recording paths

4. **Audit Logging:**
   - Log all `tui_launch` commands
   - Log all `tui_run_code` executions
   - Useful for security monitoring

5. **Dependency Scanning:**
   - Set up automated `cargo audit` in CI/CD
   - Monitor security advisories for dependencies

### Low Priority

6. **Consider Capability-Based Security:**
   - Allow users to disable certain features (e.g., JavaScript execution)
   - Provide command/path allowlists

## Security Best Practices for Users

### Safe Usage

1. **Validate Commands:** Always verify commands before using `tui_launch`
2. **Sandbox Environment:** Run in containers or VMs for untrusted use cases
3. **Restrict Permissions:** Run with minimal user privileges
4. **Monitor Sessions:** Review what TUI applications are launched
5. **Review JavaScript:** Inspect JavaScript code before using `tui_run_code`

### Unsafe Patterns to Avoid

❌ **DON'T:** Launch commands with untrusted input
```json
{"name": "tui_launch", "arguments": {"command": USER_INPUT}}
```

❌ **DON'T:** Execute untrusted JavaScript
```json
{"name": "tui_run_code", "arguments": {"code": UNTRUSTED_CODE}}
```

✅ **DO:** Validate and sanitize all inputs
✅ **DO:** Use in trusted environments
✅ **DO:** Run with principle of least privilege

## Conclusion

**Overall Security Assessment: GOOD**

The `mcp-tui-driver` project demonstrates good security practices:

1. ✅ **No unexpected network access** - Does not "phone home" or send data externally
2. ✅ **Controlled code execution** - JavaScript runtime is sandboxed with limited APIs
3. ✅ **Purpose-appropriate command execution** - Necessary for TUI automation
4. ✅ **Protected file system access** - Path traversal prevention in place
5. ✅ **Clean dependency tree** - No suspicious or unnecessary network libraries

**Primary Security Considerations:**

The application's security model assumes:
- Users trust the commands they launch via `tui_launch`
- Users trust the JavaScript they execute via `tui_run_code`
- The application runs with appropriate user permissions

For testing Textual-based apps, this tool is **safe to use** provided you:
1. Only launch trusted applications
2. Review any JavaScript code before execution
3. Run with appropriate user permissions
4. Understand that launched TUI apps have the same privileges as the driver

## Contact

For security concerns or to report vulnerabilities, please create an issue in the repository or contact the maintainers directly.

---

**Audit Performed By:** GitHub Copilot Security Analysis
**Date:** 2026-02-11
**Version Audited:** v0.1.0
