SHELL := /bin/bash
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)
CARGO ?= cargo +stable
CLIPPY_FLAGS := -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery
RUSTDOCFLAGS ?= -D warnings
ARGS ?=
