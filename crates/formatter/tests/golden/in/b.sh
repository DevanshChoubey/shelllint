#!/bin/bash
foo=$1
ls $foo
	cat $foo | grep `date`
if [ $foo == 1 ]; then echo yes; fi
rm -rf $dir/*
