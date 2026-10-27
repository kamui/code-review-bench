
### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data ; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "multipart/form-data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```
