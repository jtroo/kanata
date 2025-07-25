# Kanata Hot Reload Implementation Plan

## Overview

This document outlines the plan to implement configuration hot reloading functionality for kanata, addressing the community need for dynamic configuration updates during development. The implementation will be submitted as a series of focused pull requests to ensure easy review and incremental value delivery.

## Goals

### Primary Goals
1. **Enable IPC-based config reload** - Allow external tools to trigger configuration reloads via TCP commands
2. **Add automatic file watching** - Implement optional automatic config reloading when files change
3. **Maintain stability** - Ensure all changes are backward compatible and don't affect existing functionality
4. **Follow kanata conventions** - Match the existing code style, patterns, and architecture

### User Benefits
- Faster configuration development workflow
- Integration with external tools and editors
- Reduced friction when testing configuration changes
- Consistent with existing `lrld` keyboard actions

## Implementation Strategy

### PR #1: Basic File Watching with --watch Flag
**Timeline: 1 week**
**Goal: Core file watching functionality with automatic reload**

#### Tasks
1. Add `--watch` CLI flag:
   ```rust
   #[arg(long, help = "Watch configuration files for changes and reload automatically")]
   watch: bool,
   ```

2. Add dependencies to Cargo.toml:
   ```toml
   notify = "8.1.0"
   notify-debouncer-mini = "0.4"
   ```

3. Implement basic file watching:
   - Watch main config files in `cfg_paths` only (no includes)
   - Use fixed 500ms debounce with rate limiting
   - Only reload if changed file is currently active config
   - Leverage existing `live_reload_requested` flag mechanism

4. Update documentation in config.adoc:
   - Document `--watch` flag usage
   - Clearly state what can/cannot be hot-reloaded
   - Add examples and limitations

#### Deliverables
- Working `--watch` flag for main config files
- Cross-platform file watching (Linux/Windows/macOS)
- Complete documentation with clear limitations
- Rate limiting protection (500ms minimum between reloads)

### PR #2: TCP Commands for Remote Control
**Timeline: 3-4 days**
**Goal: Add TCP commands that mirror existing keyboard live reload actions**

#### Tasks
1. Extend `tcp_protocol` with reload commands:
   ```rust
   pub enum ClientMessage {
       ReloadConfig {},                    // Like 'lrld'
       ReloadConfigNext {},                // Like 'lrnx'
       ReloadConfigPrev {},                // Like 'lrpv'
       ReloadConfigNum { index: usize },   // Like 'lrld-num N'
       ReloadConfigFile { path: String },  // Like 'lrld-file "path"'
   }
   ```

2. Implement TCP message handlers in `tcp_server.rs`:
   - Set `live_reload_requested` flag (same as keyboard actions)
   - Send wakeup event to processing loop
   - Add rate limiting for TCP commands

3. Update example TCP client with reload demonstrations

4. Add tests for TCP reload commands

#### Deliverables
- Complete TCP command parity with keyboard actions
- Updated example client demonstrating all reload commands
- Full test coverage for TCP commands
- Updated documentation with TCP examples

### PR #3: Include File Support
**Timeline: 4-5 days**
**Goal: Extend file watching to monitor included files**

#### Tasks
1. Parse config files to extract `(include ...)` statements during watcher setup

2. Build file dependency tracking:
   - Map included files back to their parent configs
   - Handle multiple configs including the same file
   - Avoid infinite loops with nested includes

3. Extend file watching to include files:
   - Watch all included files in addition to main configs
   - Reload appropriate parent config when include changes
   - Handle include file additions/deletions

4. Add comprehensive tests for include scenarios

#### Deliverables
- Complete file watching including dependency tracking
- Reliable include file change detection
- Test coverage for complex include scenarios
- Updated documentation with include file behavior

## Technical Considerations

### Architecture Principles
1. **Leverage existing patterns** - Use the existing `live_reload_requested` flag mechanism
2. **Minimal invasive changes** - Extend rather than modify core functionality
3. **Thread safety** - Maintain existing `Arc<Mutex<Kanata>>` patterns
4. **Channel-based communication** - Use existing channel patterns for cross-thread events

### Code Style Guidelines
- Match existing indentation and formatting
- Use similar error handling patterns (`bail!`, `log::error!`)
- Follow existing naming conventions (e.g., `CustomAction::LiveReload*`)
- Maintain existing comment style and documentation patterns

### Testing Strategy
1. **Unit tests** for new functionality
2. **Integration tests** for TCP commands
3. **Manual testing** across all platforms
4. **Example configurations** demonstrating new features

### Risk Mitigation
1. **Feature flags** - All new functionality behind opt-in flags
2. **Graceful degradation** - System continues working if new features fail
3. **Backward compatibility** - No changes to existing behavior unless explicitly enabled
4. **Incremental delivery** - Each PR provides standalone value

### Security Considerations
1. **Same attack surface** - TCP commands leverage existing server infrastructure (localhost-only binding)
2. **Rate limiting** - Prevent reload command spam (500ms minimum interval between reloads)
3. **File permissions** - File watching respects existing OS file permissions
4. **Opt-in only** - `--watch` flag required, TCP server must be explicitly enabled
5. **Existing validation** - Config parser already handles untrusted input safely

## Success Metrics

### Functional Success
- [ ] TCP reload commands work reliably
- [ ] File watching works on all supported platforms
- [ ] Error reporting provides actionable information
- [ ] Include files are properly watched

### Quality Metrics
- [ ] No performance regression in key processing
- [ ] Memory usage increase < 1MB
- [ ] All tests passing
- [ ] Documentation complete and clear

### Community Acceptance
- [ ] PRs reviewed and approved efficiently
- [ ] Features align with maintainer vision
- [ ] Community finds features valuable
- [ ] No breaking changes for existing users

## Communication Plan

### PR Descriptions
Each PR will include:
1. **Problem statement** - Why this change is needed
2. **Solution overview** - High-level approach taken
3. **Testing performed** - Platforms tested, edge cases considered
4. **Documentation updates** - What docs were updated
5. **Future work** - What comes next in the plan

### Community Engagement
1. **Discussion thread** - Link back to original GitHub discussion
2. **Incremental value** - Emphasize that each PR provides standalone benefit
3. **Feedback incorporation** - Be responsive to maintainer and community input


### Future Enhancements (Not in Current Scope)

1. **Selective reload** - Only reload changed portions of config
2. **Config validation endpoint** - TCP command to validate without reloading
3. **Watch pattern configuration** - Allow custom file patterns for watching
4. **Reload history** - Track and revert to previous configurations

## Conclusion

This plan provides a clear path to implementing hot reload functionality in kanata through a series of focused, reviewable pull requests. Each phase delivers immediate value while building toward the complete feature set. The implementation respects kanata's existing architecture and patterns while addressing a real community need.
