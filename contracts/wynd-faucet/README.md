# Wynd Faucet

You can fill it with tokens and it will let users request them. Once each.

## Usage

Instantiate (`max_requests` defaults to 1 if omitted):

```json
{
  "token": "juno1djh30u03f03hf8h38tg32",
  "amount": "123000000",
  "max_requests": 3
}
```

Execute:

```json
{ "request_funds": {} }
```

**Query:**

1. Get the balance of the faucet itself:

```json
{ "balance": {} }
```

Returns:

```json
{ "balance": "45678900000" }
```


2. Get the config of the faucet:

```json
{ "config": {} }
```

Returns:

```json
{
  "token": "juno1djh30u03f03hf8h38tg32",
  "amount": "123000000",
  "max_requests": 3
}
```


3. See how many times the given address has called the faucet:

```json
{
  "calls": {
    "address": "juno1djh30u03f03hf8h38tg32"
  }
}
```

Returns:

```json
{ "calls": 1 }
```

## Building

