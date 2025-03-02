print("in kb-test")

print("getting kb")
local kb = wrought_kb()
print("got kb")

-- Load all the files into the KB
print("reading file")
local f1 = read_file("research/fantasy_tropes.md")
print(f1)

local doc = kb:doc_from_content(f1)
doc = doc:tagged("research")
doc:ingest()
