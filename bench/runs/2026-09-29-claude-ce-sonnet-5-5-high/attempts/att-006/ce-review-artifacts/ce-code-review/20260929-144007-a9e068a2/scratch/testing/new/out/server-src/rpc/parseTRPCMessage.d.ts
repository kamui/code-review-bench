import type { CombinedDataTransformer } from '../transformer';
import type { TRPCClientOutgoingMessage } from './envelopes';
/** @public */
export declare function parseTRPCMessage(obj: unknown, transformer: CombinedDataTransformer): TRPCClientOutgoingMessage;
