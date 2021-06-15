#! /usr/bin/bash -x
# [[file:../runners.note::*test runner][test runner:1]]
sleep 5 &
sleep 10 &
sleep 2 &
pstree -p $$
pgrep -s $$

echo 'parent exit'
exit 0
# test runner:1 ends here
