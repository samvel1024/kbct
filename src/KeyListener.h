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
#include "poll/Subscriber.h"


class KeyListener : public Subscriber {
private:

	int keyboard_fd;
	std::string device;
	std::function<void(std::vector<struct input_event> &)> callback;
	std::function<void(void)> on_should_unsubscibe;

public:

	static int test_grab(int fd, int grab_flag) {
		int rc;
		rc = ioctl(fd, EVIOCGRAB, (void *) 1);
		if (rc == 0 && !grab_flag)
			ioctl(fd, EVIOCGRAB, (void *) 0);
		return rc;
	}

public:

	KeyListener(std::string &device, std::function<void(void)> on_unsub,
	            std::function<void(std::vector<struct input_event> &)> callback)
			: Subscriber(device), callback(std::move(callback)), keyboard_fd(0), device(device), on_should_unsubscibe(std::move(on_unsub)) {
		std::cout << "Iniitializing keyboard listener for " << device << std::endl;
		if ((keyboard_fd = open(device.c_str(), O_RDONLY)) < 0) {
			std::cout << errno << std::endl;
			if (errno == EACCES && getuid() != 0) {
				std::cerr << "Cannot access " << device << ". Try running as root" << std::endl;
			}
			throw std::exception();
		}

		if (test_grab(keyboard_fd, 1)) {
			std::cerr << "Device is grabbed by another process" << std::endl;
			throw std::exception();
		}

		set_fd(keyboard_fd);
		set_expected(POLLIN | POLLERR);
	}

	~KeyListener() override {
		if (keyboard_fd > 0) {
			std::cout << "Ungrabbing device " << device << std::endl;
			ioctl(keyboard_fd, EVIOCGRAB, (void *) 0);
		}
	}

	void on_error(Poll &p, int event) override {
		Subscriber::on_error(p, event);
	}

	void on_output(Poll &p) override {
		Subscriber::on_output(p);
	}

	void on_input(Poll &p) override {
		std::vector<struct input_event> ev(64);
		int rd = read(keyboard_fd, ev.data(), ev.size() * sizeof(struct input_event));

		if (rd == -1) {
			std::cout << "Device " << device << " is disconnected" << std::endl;
			// This will unsubscribe from poll as well
			on_should_unsubscibe();
			return;
		}
		if (rd < sizeof(struct input_event)) {
			std::cerr << "Read invalid length" << std::endl;
			throw std::exception();
		}
		int size = rd / sizeof(struct input_event);
		std::vector<struct input_event> ev_copy(ev.begin(), ev.begin() + size);
		callback(ev_copy);

	}


};


#endif //LAYKEYMAP_KEYLISTENER_H
