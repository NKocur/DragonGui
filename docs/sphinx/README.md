# DragonGUI Sphinx Docs

This folder contains the Sphinx documentation site for DragonGUI.

Install documentation dependencies from the repository root:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe -m pip install -e .[docs]
```

Build HTML:

```powershell
$env:PYTHONPATH = "python"
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe -m sphinx -b html docs\sphinx docs\sphinx\_build\html
```

Open:

```text
docs/sphinx/_build/html/index.html
```

The source currently mixes new guide pages with links back to the existing
root-level Markdown notes. As the docs mature, stable content should move into
this Sphinx tree.

