#include "Error.h"

Error::Error(char const *fmt, ...) {
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(text, sizeof text, fmt, ap);
	va_end(ap);
}

char const *Error::what() const noexcept {
	return this->text;
}

std::string from_errno() {
	std::string out = std::string("syserror{code = ") + std::to_string(errno) + ", msg = " + strerror(errno) + "}";
	return out;
}

int no_err(int val, const char *msg) {
	if (val < 0) {
		throw Error("%s, %s\n", from_errno().c_str(), msg);
	}
	return val;
}
