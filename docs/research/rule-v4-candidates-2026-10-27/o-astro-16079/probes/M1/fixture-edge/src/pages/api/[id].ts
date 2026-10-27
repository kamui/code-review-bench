export async function GET({ params, request }) {
	return Response.json({ route: 'api/[id]', id: params.id, requestUrl: request.url });
}
