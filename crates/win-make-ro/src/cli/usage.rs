pub const USAGE: &str = "\
usage:
  win-make-ro lock   [--gui] [--no-elevate] <path>...   make read only (recursive)
  win-make-ro unlock [--gui] [--no-elevate] <path>...   remove our read only (recursive)
  win-make-ro status <path>...                          print lock state per path
  win-make-ro install                                   register the context menu (current user)
  win-make-ro uninstall                                 remove the context menu

  --gui         report errors in a message box instead of stderr
  --no-elevate  never re-launch elevated on access denied
  --            end of options

note: npm runs no scripts on uninstall, so if this came from npm, run
      `win-make-ro uninstall` before `npm uninstall -g win-make-ro`.";
