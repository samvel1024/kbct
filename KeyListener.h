#ifndef LAYKEYMAP_KEYLISTENER_H
#define LAYKEYMAP_KEYLISTENER_H

#include <functional>
#include <utility>
#include <vector>
#include <fcntl.h>
#include <zconf.h>
#include <iostream>
#include <linux/input.h>
#include <csignal>
#include <sys/signalfd.h>

class KeyListener {
private:

	int keyboard_fd;
	std::string device;
	std::function<void(std::vector<struct input_event> &)> callback;

public:

	static int test_grab(int fd, int grab_flag) {
		int rc;
		rc = ioctl(fd, EVIOCGRAB, (void *) 1);
		if (rc == 0 && !grab_flag)
			ioctl(fd, EVIOCGRAB, (void *) 0);
		return rc;
	}

	static int signal_fd() {
		sigset_t mask;

		sigemptyset(&mask);
		sigaddset(&mask, SIGTERM);
		sigaddset(&mask, SIGINT);

		if (sigprocmask(SIG_BLOCK, &mask, NULL) < 0) {
			std::cerr << "Failed sigprocmask" << std::endl;
			throw std::exception();
		}

		int sfd = signalfd(-1, &mask, 0);
		if (sfd < 0) {
			std::cerr << "Failed signalfd" << std::endl;
			throw std::exception();
		}
		return sfd;
	}

	void initialize(const std::string &filename) {
		std::cout << "Iniitializing keyboard listener for " << filename << std::endl;
		if ((keyboard_fd = open(filename.c_str(), O_RDONLY)) < 0) {
			std::cout << errno << std::endl;
			if (errno == EACCES && getuid() != 0) {
				std::cerr << "Cannot access " << filename << ". Try running as root" << std::endl;
			}
			perror("evtest");
			throw std::exception();
		}

		if (test_grab(keyboard_fd, 1)) {
			std::cerr << "Device is grabbed by another process" << std::endl;
			throw std::exception();
		}
	}

public:

	KeyListener(std::string &device, std::function<void(std::vector<struct input_event> &)> callback)
			: device(device), callback(std::move(callback)), keyboard_fd(0) {
		initialize(this->device);
	}

	virtual ~KeyListener() {
		if (keyboard_fd > 0) {
			std::cout << "Ungrabbing device " << device << std::endl;
			ioctl(keyboard_fd, EVIOCGRAB, (void *) 0);
		}
	}

	void add_fd(int fd, fd_set &fds, int &max_fd) {
		FD_SET(fd, &fds);
		if (fd > max_fd) {
			max_fd = fd;
		}
	}

	void listen_keystrokes() {
		std::vector<struct input_event> ev(64);

		int max_fd = 0;
		int sig_fd = signal_fd();

		fd_set master;
		FD_ZERO(&master);
		add_fd(keyboard_fd, master, max_fd);
		add_fd(sig_fd, master, max_fd);

		while (true) {

			fd_set fds = master;
			if (select(max_fd + 1, &fds, NULL, NULL, NULL) < 0) {
				std::cerr << "Error in select" << std::endl;
				throw std::exception();
			}

			// Check if signal is received
			if (FD_ISSET(sig_fd, &fds)) {
				break;
			}

			int rd = read(keyboard_fd, ev.data(), ev.size() * sizeof(struct input_event));

			if (rd < (int) sizeof(struct input_event)) {
				std::cerr << "Read invalid length" << std::endl;
				throw std::exception();
			}

			int size = rd / sizeof(struct input_event);
			std::vector<struct input_event> ev_copy(ev.begin(), ev.begin() + size);
			callback(ev_copy);
		}
		std::cerr << "Exiting" << std::endl;
	}


};


#endif //LAYKEYMAP_KEYLISTENER_H
