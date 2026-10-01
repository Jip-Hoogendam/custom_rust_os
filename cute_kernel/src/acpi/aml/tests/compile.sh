#!/bin/bash

mkdir compiled
postf=.aml

for i in *.asl; do
  name="${i%$postf}"
  iasl -p./compiled/$i $name
done
