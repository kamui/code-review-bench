import { Hono } from '<scratch>/trees/base/src/index'
import { methodOverride } from '<scratch>/trees/base/src/middleware/method-override/index'
import { prettyJSON } from '<scratch>/trees/base/src/middleware/pretty-json/index'
import { run } from './probe'

await run({ Hono, methodOverride, prettyJSON }, 'base')
