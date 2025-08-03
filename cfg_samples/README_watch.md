# File Watching Example

This directory contains `watch_example.kbd`, a configuration file designed to demonstrate kanata's file watching feature.

## Quick Start

1. **Build kanata with watch feature:**
   ```bash
   cargo build --features watch
   ```

2. **Run with file watching enabled:**
   ```bash
   ./target/debug/kanata --cfg cfg_samples/watch_example.kbd --watch --debug
   ```

3. **Edit and experiment:**
   - Open `watch_example.kbd` in your favorite editor
   - Modify key mappings, timings, or add new layers
   - Save the file
   - Watch kanata automatically reload in the terminal!

## What This Example Demonstrates

- **Manual reload key**: `caps` key is mapped to `lrld` for manual reloading
- **Home row mods**: A/S/D/F keys with tap-hold modifiers
- **Layer switching**: Q key with tap-hold to access development layer
- **Development workflow**: Comments explain how to experiment effectively

## Development Tips

- Use `--debug` flag to see detailed reload messages
- Invalid configurations won't reload - check console for parsing errors
- Changes are automatically debounced (500ms delay)
- Press Ctrl+Space+Esc to exit kanata safely

## Common Experiments to Try

1. **Timing adjustments**: Change tap-hold values from 200ms to 150ms or 250ms
2. **Key swapping**: Move common keys to more comfortable positions  
3. **New layers**: Add gaming or symbols layers
4. **Complex actions**: Try macros, unicode output, or command execution
5. **Modifier combinations**: Experiment with different modifier placements

## File Watching Limitations

- Only watches the main config file (not includes yet - coming in PR #3)
- File must remain valid kanata syntax to reload successfully
- Device-related configurations require full restart
- Rapid changes may be batched due to debouncing

Happy configuring! 🔥