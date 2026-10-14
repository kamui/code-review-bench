export async function GET({ params, request }) {
	const url = new URL(request.url);
	return Response.json({
		route: 'api/[id]',
		id: params.id,
		requestUrl: request.url,
		search: url.search,
		x: url.searchParams.get('x'),
	});
}
