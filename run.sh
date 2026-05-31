#!/bin/bash

cd /home/zjp/KMiri/kmiri
./miri install --debug

cd /home/zjp/KMiri/tests/init
OSDK_LOCAL_DEV=1 cargo osdk miri test
