#include <stdarg.h>
#include <stdio.h>
#include <curses.h>

#include "rogue.h"

extern int rogue_msg_str(char *text);
extern void rogue_addmsg_str(char *text);

int
msg(char *fmt, ...)
{
    va_list ap;
    char buf[MAXSTR];

    if (fmt == NULL || *fmt == '\0')
        return rogue_msg_str(fmt);

    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    buf[sizeof(buf) - 1] = '\0';

    return rogue_msg_str(buf);
}

void
addmsg(char *fmt, ...)
{
    va_list ap;
    char buf[MAXSTR];

    if (fmt == NULL || *fmt == '\0')
    {
        rogue_addmsg_str(fmt);
        return;
    }

    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    buf[sizeof(buf) - 1] = '\0';

    rogue_addmsg_str(buf);
}