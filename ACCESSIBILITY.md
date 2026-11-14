# Accessibility

FluxCode should be usable by as many people as possible. It is early for
that work, so this file says plainly what works today, what does not, and how
to report a barrier.

## What works today

- **Keyboard.** The main workflows have keyboard shortcuts, listed in the
  README under Shortcuts: the Command Palette (`Ctrl+Shift+P`), Go to File
  (`Ctrl+P`), Go to Line (`Ctrl+G`), tabs, the terminal, the agent panel
  (`Ctrl+L`), and Settings (`Ctrl+,`).
- **Text size.** The editor font size can be set from 8 to 32 pixels in
  Settings, or changed with `Ctrl+=`, `Ctrl+-`, and `Ctrl+0`. There are three
  interface sizes in Settings.
- **Themes and contrast.** There are six color themes, light and dark
  (FluxDark, FluxCream, FluxMono, FluxRed, FluxMuara, FluxMuara Dark). A test
  holds every theme to 4.5:1 contrast for text on every surface, and 5.1:1 on
  an accent fill, so a theme that ships unreadable fails the build instead of
  a reader.
- **Language.** The interface comes in English and Bahasa Indonesia.

## Known limitations

- **Screen readers.** Screen reader support has not been tested, and we do
  not claim it works yet.
- **No audit.** No accessibility audit of FluxCode has been done.
- **Some parts may still need a mouse.** Not every dialog and panel has been
  checked for keyboard-only use.

## Reporting a barrier

Open an issue through the
[bug report form](https://github.com/MuaraAI/FluxCode/issues/new/choose) and
say it is an accessibility barrier. Include the build ID (Help > About
FluxCode copies it), the operating system, what you tried to do, and what
stopped you.

Accessibility reports are treated like bug reports: maintainers reproduce
them, fix them on `main`, and ship the fix in the next BuildDev build.
