pub const USAGE: &str = "\
usage:
  win-make-ro lock   [--gui] [--no-elevate] <path>...   make read only (recursive)
  win-make-ro unlock [--gui] [--no-elevate] <path>...   remove our read only (recursive)
  win-make-ro status <path>...                          print lock state per path
  win-make-ro install                                   register the context menu (current user)
  win-make-ro reinstall                                 replace the installed copy, restart Explorer
  win-make-ro uninstall                                 remove the context menu

  --gui                report errors in a message box instead of stderr
  --no-elevate         never re-launch elevated on access denied
  --paths-from <file>  take the whole selection from a file and leave the file
                       alone; for a selection too large for a command line.
                       The file is UTF-16LE with no BOM, one NUL between
                       entries and none at the end. Cannot be combined with
                       paths given on the command line.
  --consume-paths-from <file>
                       the same, but the file is removed once read. The shell
                       extension hands its own one-shot list over this way.
  --                   end of options

note: npm runs no scripts on uninstall, so if this came from npm, run
      `win-make-ro uninstall` before `npm uninstall -g win-make-ro`.";
