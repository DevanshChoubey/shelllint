| input | lines | Haskell time (s) | Rust time (s) | speedup | Haskell RSS (MB) | Rust RSS (MB) | RSS ratio | diagnostics (line,col,code) |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| build.sh | 80 | 0.026 | 0.007 | 3.5x | 16.8 | 8.1 | 2.1x | exact |
| installer.sh | 93 | 0.043 | 0.009 | 4.7x | 21.9 | 7.9 | 2.8x | exact |
| long_case.sh | 2811 | 2.753 | 0.174 | 15.9x | 1897.6 | 25.8 | 73.6x | exact |
| corpus_concat.sh | 2338 | 2.870 | 0.173 | 16.6x | 196.6 | 27.3 | 7.2x | exact |
| gen_10k.sh | 10000 | 40.409 | 0.758 | 53.3x | 645.8 | 93.9 | 6.9x | exact |
| gen_1k.sh | 1000 | 0.873 | 0.080 | 10.9x | 94.7 | 14.6 | 6.5x | exact |
| gen_50k.sh | 50000 | timeout | 4.320 | - | - | 441.7 | - | n/a |
| patho_1mb_line.sh | 1 | 1.402 | 0.170 | 8.3x | 224.5 | 16.0 | 14.0x | exact |
| patho_50k_lines.sh | 50000 | timeout | 3.823 | - | - | 350.9 | - | n/a |
| patho_nested_5000.sh | 1 | exit 137 | 0.942 | - | - | 189.6 | - | n/a |

- build.sh: hs=51 rs=51 onlyHS=[] onlyRS=[]
- installer.sh: hs=86 rs=86 onlyHS=[] onlyRS=[]
- long_case.sh: hs=2402 rs=2402 onlyHS=[] onlyRS=[]
- corpus_concat.sh: hs=4593 rs=4593 onlyHS=[] onlyRS=[]
- gen_10k.sh: hs=21148 rs=21148 onlyHS=[] onlyRS=[]
- gen_1k.sh: hs=2205 rs=2205 onlyHS=[] onlyRS=[]
- patho_1mb_line.sh: hs=1 rs=1 onlyHS=[] onlyRS=[]
