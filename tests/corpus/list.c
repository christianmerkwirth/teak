#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct node {
    int value;
    struct node *next;
};

static struct node *push(struct node *head, int value)
{
    struct node *n = malloc(sizeof(*n));
    if (!n) {
        perror("malloc");
        exit(1);
    }
    n->value = value;
    n->next = head;
    return n;
}

int main(void)
{
    struct node *list = NULL;
    for (int i = 0; i < 10; i++)
        list = push(list, i);
    for (struct node *n = list; n; n = n->next)
        printf("%d\n", n->value);
    return 0;
}
