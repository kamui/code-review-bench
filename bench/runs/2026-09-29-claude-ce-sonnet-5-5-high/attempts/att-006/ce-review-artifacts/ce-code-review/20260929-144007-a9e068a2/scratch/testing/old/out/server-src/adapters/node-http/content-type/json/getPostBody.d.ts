import { BodyResult } from '../../../../http/contentType';
import { NodeHTTPRequest } from '../../types';
export declare function getPostBody(opts: {
    req: NodeHTTPRequest;
    maxBodySize?: number;
}): Promise<BodyResult>;
