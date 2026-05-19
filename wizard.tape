Output docs/src/wizard.gif
Set FontSize 14
Set Width 850
Set Height 600
Set Padding 20
Set Theme "Dracula"
Set TypingSpeed 50ms

Hide
Type "cargo build -p zarrduck > /dev/null 2>&1"
Enter
Type "clear"
Enter
Show

Type "./target/debug/zarrduck plot ../../analysis.duckdb"
Sleep 500ms
Enter
Sleep 1.5s
# Select table (extracted_data)
Enter
Sleep 1.5s
# Select variables
Down 3
Sleep 500ms
Space
Sleep 500ms
Enter
Sleep 1.5s
# Plot type (Histogram)
Enter
Sleep 1.5s
# Group by (None)
Enter
Sleep 4s
