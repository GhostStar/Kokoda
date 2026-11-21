#ifndef KOKODA_CORE_H
#define KOKODA_CORE_H
// UI-thread confined. Returned strings belong to Rust and must be released here.
void *kokoda_client_new(void);
void kokoda_client_free(void *client);
char *kokoda_client_dispatch(void *client, const char *json);
void kokoda_client_string_free(char *json);
#endif
