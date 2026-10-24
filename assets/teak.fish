complete -c teak -s n -l lines -d 'Line budget: a number (3 or more), or auto (terminal height - 2) [default: 100]' -r
complete -c teak -s L -l depth -d 'Never open deeper than N' -r
complete -c teak -l sort -d 'Display order inside a directory' -r -f -a "salience\t''
name\t''
size\t''
age\t''
type\t''"
complete -c teak -l focus -d 'Spend most of the budget on PATH, keep the context around it' -r -F
complete -c teak -l series-min -d 'Smallest group that becomes a series row [default: 4]' -r
complete -c teak -l detect -d 'Content detection [default: full]' -r -f -a "off\t''
fast\t''
full\t''"
complete -c teak -l preview -d 'Previews [default: short]' -r -f -a "off\t''
short\t''
full\t''"
complete -c teak -l templates -d 'Add boilerplate templates from DIR (boilerplate/ and generated/)' -r -F
complete -c teak -l dump-templates -d 'Write the built-in templates to DIR and exit' -r -F
complete -c teak -l chaff -d 'Set the action for all chaff classes' -r -f -a "show\t''
dim\t''
fold\t''
hide\t''"
complete -c teak -l show -d 'Show these chaff classes as normal rows' -r -f -a "vendor\t''
build\t''
cache\t''
vcs\t''
editor\t''
lock\t''
generated\t''
minified\t''
boilerplate\t''
empty\t''
duplicate\t''
ignored\t''"
complete -c teak -l fold -d 'Fold these chaff classes into the chaff line' -r -f -a "vendor\t''
build\t''
cache\t''
vcs\t''
editor\t''
lock\t''
generated\t''
minified\t''
boilerplate\t''
empty\t''
duplicate\t''
ignored\t''"
complete -c teak -l hide -d 'Count these chaff classes only in the footer' -r -f -a "vendor\t''
build\t''
cache\t''
vcs\t''
editor\t''
lock\t''
generated\t''
minified\t''
boilerplate\t''
empty\t''
duplicate\t''
ignored\t''"
complete -c teak -l format -d 'Output format (default: tty on a terminal, else plain)' -r -f -a "tty\t''
plain\t''
md\t''
json\t''"
complete -c teak -l color -d 'Colors [default: auto]' -r -f -a "auto\t''
always\t''
never\t''"
complete -c teak -l width -d 'Output width (default: the terminal width, or 120)' -r
complete -c teak -l time -d 'Age format [default: relative]' -r -f -a "relative\t''
iso\t''"
complete -c teak -l why -d 'Explain the decision for PATH' -r -F
complete -c teak -s j -l threads -d 'Walk threads' -r
complete -c teak -s l -l long -d 'All columns (ls -lah style)'
complete -c teak -l dupes -d 'Find duplicate files: fold copies of visible files'
complete -c teak -l mismatches -d 'List only files whose content does not match the extension'
complete -c teak -l no-cache -d 'Do not read or write the type cache'
complete -c teak -l templates-only -d 'Use only the templates from --templates'
complete -c teak -s A -l almost-all -d 'Show hidden entries (dotfiles, .git/), folded into the chaff line'
complete -c teak -s a -l all -d 'Show everything: no chaff folding, no hidden entries, no gitignore'
complete -c teak -l no-gitignore -d 'Do not use .gitignore rules'
complete -c teak -s x -l one-file-system -d 'Do not cross into other file systems'
complete -c teak -l follow -d 'Follow symlinks'
complete -c teak -l json-full -d 'JSON with every entry (implies --format json)'
complete -c teak -l ascii -d 'ASCII glyphs only'
complete -c teak -l bytes -d 'Sizes in whole bytes'
complete -c teak -l si -d 'Sizes in SI units (base 1000)'
complete -c teak -l no-git -d 'No git status'
complete -c teak -l stats -d 'Print only the header, the type table, and the footer'
complete -c teak -l version -d 'Print the version'
complete -c teak -s h -l help -d 'Print help'
