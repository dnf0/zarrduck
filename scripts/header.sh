#!/bin/bash
function header() { 
  printf '\033[1A\033[2K\r\n\033[1;36m# %s\033[0m\n' "$*"
}
