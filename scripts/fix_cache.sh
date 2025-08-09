#!/bin/sh

grep -rlZ "429 Too Many Requests" . | xargs -0 rm -f
find . -type f -empty -delete