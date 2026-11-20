# Command-line launcher

Install the `gitrune` command from **GitRune → Install 'gitrune' Command** on
macOS, or **Tools → Install 'gitrune' Command** on Windows and Linux. The same
action is in Find Action. Open a new terminal after installing.

- **macOS:** move GitRune to Applications first. The command links to the launcher
  inside the app bundle. It uses `~/.local/bin` when that directory is on PATH;
  otherwise it uses `/usr/local/bin` and may request an administrator password.
- **Windows:** installation creates `gitrune.cmd` in
  `%LOCALAPPDATA%\GitRune\bin` and adds that directory to the user PATH. This
  works with installed and portable copies, without administrator rights.
- **Linux:** installation creates `~/.local/bin/gitrune`. If another command
  takes precedence or this directory is absent from PATH, the notification asks
  you to put it at the beginning of your shell's PATH. For Bash and Zsh, add
  `export PATH="$HOME/.local/bin:$PATH"` to your shell profile. For Fish, run
  `fish_add_path --prepend ~/.local/bin`. An AppImage launcher targets the original
  AppImage; keep that file in a permanent location.

```sh
gitrune <path>[:line[:column]] ...   # open folders and files
gitrune --line 12 notes.md           # caret at line 12
gitrune diff a.txt b.txt             # open the comparison window
gitrune --help
```

Folders open as projects; files open in LightEdit unless they belong to an open
project. Arguments that start with `-` follow the flags above rather than being
treated as paths.
