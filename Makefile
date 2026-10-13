.PHONY: wiki-init wiki-check wiki-prepare

wiki-init:
	sh scripts/wiki/init.sh

wiki-check:
	scripts/dev.sh python3 scripts/wiki/check.py

wiki-prepare:
	sh scripts/wiki/prepare.sh "$(BRANCH)"
