# Reactive API Examples

These files started as design fixtures for the reactive-native API. They are
now runnable examples for the current component/runtime implementation while
still documenting the API shape DragonGUI is aiming for.

## Examples

- `01_simple_scatter_component.py`: one component owns dropdown state and drives
  a scatter widget.
- `02_nested_dataframe_component.py`: a parent passes a DataFrame to child
  components; each child owns local state.
- `03_background_scatter_updates.py`: a background thread pushes new scatter
  data through the command queue.

Run them from the repository root:

```powershell
python plans\10-reactive-native-engine\api-examples\01_simple_scatter_component.py
python plans\10-reactive-native-engine\api-examples\02_nested_dataframe_component.py
python plans\10-reactive-native-engine\api-examples\03_background_scatter_updates.py
```
