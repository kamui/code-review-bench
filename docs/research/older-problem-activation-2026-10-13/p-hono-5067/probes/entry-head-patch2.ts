import { Hono } from '<scratch>/trees/head-patch2/src/index'
import { methodOverride } from '<scratch>/trees/head-patch2/src/middleware/method-override/index'
import { prettyJSON } from '<scratch>/trees/head-patch2/src/middleware/pretty-json/index'
import { run } from './probe'

await run({ Hono, methodOverride, prettyJSON }, 'head-patch2')
