export async function GET({ params, request }) {
	return Response.json({ route: 'api/[id]', method: 'GET', id: params.id, requestUrl: request.url });
}

export async function POST({ params, request }) {
	return Response.json({
		route: 'api/[id]',
		method: 'POST',
		id: params.id,
		body: await request.text(),
	});
}
