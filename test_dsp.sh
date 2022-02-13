#!/bin/bash

for file in ~/games/snes/*; do
    if [[ -f "$file" ]]; then
        #echo "$file"
        case "$file" in
            *.smc|*.sfc)
                cargo run --release -- "$file" >& /dev/null
                if [[ $? -eq 5 ]]; then
                    echo "$file"
                fi
                ;;
            *)  ;;
        esac
    fi
done
