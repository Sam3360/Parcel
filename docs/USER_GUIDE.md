# Parcel user guide

## The idea

A **workspace** is a place to gather things before you package them. You add files, folders, notes and links; Parcel *remembers where files are* (it does not copy or move them). When you **export**, Parcel reads those files and writes a single `.parcel` file containing everything, with checksums so anyone can verify it later.

## Getting started

1. **Home → name a workspace → Create** (or *From a template…*).
2. **Add** items: use *Add → Files…* or *Folder from disk…*, or drag files and folders from Explorer into the window.
3. **Organise**: *Add → New folder…*, drag items onto folders in the left panel, or right-click → *Move to…*.
4. **Describe**: select an item and use the right panel — description, tags, custom metadata, dependencies. Add Markdown notes (*Add → New note…*, or *Edit workspace notes*) and references (References tab).
5. **Export package** (Ctrl+E). Choose a profile, then where to save.

## Workspace window

* **Left:** folder structure and tags. Drop items on a folder to move them.
* **Centre:** items in list or grid view. Tabs: *Items*, *References*, *Health*.
* **Right:** the inspector — details, preview, description, tags, metadata. With nothing selected it shows workspace statistics and notes.

Removing an item (Delete) only removes it from the workspace. **Your original files are never deleted or modified.**

## Search

Type in the search box (Ctrl+F). Plain words match names, descriptions, tags, metadata and note text. Operators: `kind:note`, `tag:finance`, `ext:pdf`. The type dropdown and tag list in the left panel filter further.

## Health tab

* **Source files** — *Check sources* compares each file with what was recorded (size and modified time; *Deep check* re-reads and compares checksums). Missing files can be *relinked*; changed files can be *accepted* as the new version. Export is blocked until problems are resolved or explicitly accepted.
* **Duplicate content** — files with identical bytes, with an option to drop the extra copies from the workspace.
* **Dependency checks** and **Revisions** (every export is a numbered revision).

## Opening packages

*Open Package* (Ctrl+O) works without any workspace. You can browse folders, search, preview files, read notes and references, **Verify integrity**, **Extract** everything or a selection, **Import as workspace** to edit it, or **Compare** it with an older version. If a package looks unsafe or damaged, Parcel refuses to open it and tells you why; nothing is extracted or run.

When extracting, pick what happens if a file already exists: stop before writing anything (default), keep both, or skip existing. Parcel never overwrites silently.

## Importing a ZIP

*Home → Import ZIP…* converts a ZIP archive into a new `.parcel` (the ZIP is untouched).

## Templates and export profiles

*Templates* offers five starters (Research, Design handoff, Software project, School assignment, General documents), lets you create your own or save a workspace's structure as a template, and manage **export profiles** (compression, verification, files to exclude such as `*.tmp`, suggested file name).

## Settings

Theme, density, default view, reduced motion, confirmation prompts, and **security limits** for opening packages (see [SECURITY.md](SECURITY.md)). Parcel stores its data in your user profile (`%APPDATA%\org.parcel.app`); nothing leaves your computer.

## Keyboard shortcuts

Press `?` anywhere. Highlights: Ctrl+1…6 navigate, Ctrl+O open package, Ctrl+F search, Ctrl+N new note, Ctrl+Shift+N new folder, Ctrl+Shift+A add files, Ctrl+E export, F2 rename, Delete remove, Enter open, Backspace up, arrows move selection.
