all:
	cmake -B build
	make -C build -j16

clean:
	rm -rf build

install:
	cp build/src/keyboard_server ${HOME}/.local/bin
	cp build/src/apm_client ${HOME}/.local/bin

.PHONY:
	all clean
