#! /usr/bin/bash -x
# [[file:../runners.note::*test runner][test runner:1]]
sleep 50 &
sleep 10 &
sleep 20 &
pstree -p $$
pgrep -s $$

wait
# echo 'parent exit'
exit 0
# test runner:1 ends here
