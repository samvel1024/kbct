#ifndef DISTFS_NIOUTIL_H
#define DISTFS_NIOUTIL_H

#include <vector>
#include <memory>
#include <unordered_map>
#include <system_error>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <fcntl.h>
#include <netdb.h>
#include <string>
#include <iostream>
#include <poll.h>
#include <stdarg.h>
#include <sys/ioctl.h>

struct Error : std::exception {
	char text[1000];

	explicit Error(char const *fmt, ...) __attribute__((format(printf, 2, 3)));

	char const *what() const noexcept override;
};

struct IllegalPacket : std::exception {
};

std::string from_errno();

int no_err(int val, const char *msg);

#endif
