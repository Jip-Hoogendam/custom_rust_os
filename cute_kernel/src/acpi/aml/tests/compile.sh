#!/bin/bash

mkdir compiled
postf=.asl
prefix=test_cases/

for i in test_cases/*.asl; do
  string="${i%$postf}"
  name="${string#$prefix}"
  iasl -oa -f -p./compiled/$name $i
done
