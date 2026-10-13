func (s) TestInlineCallbackInBuildModified(t *testing.T) {
	var gsb, tcc setupResult
	tcc, gsb = setup(t)
	// This build call should cause all of the inline updates to forward to the
	// ClientConn.
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case new_state := <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case new_subconn := <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case update_addrs := <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case shutdown_subconn := <-tcc.ShutdownSubConnCh:
	}

	oldCurrent := gsb.balancerCurrent.Balancer.(*buildCallbackBal)

	// Since the callback reports a state READY, this new inline balancer should
	// be swapped to the current.
	ctx, cancel = context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	gsb.SwitchTo(buildCallbackBalancerBuilder{})
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case new_state := <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case new_subconn := <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case update_addrs := <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case shutdown_subconn := <-tcc.ShutdownSubConnCh:
	}

	// The current balancer should be closed as a result of the swap.
	if err := oldCurrent.waitForClose(ctx); err != nil {
		t.Fatalf("error waiting for balancer close: %v", err)
	}

	// The old balancer should be deprecated and any calls from it should be a no-op.
	oldCurrent.newSubConn([]resolver.Address{}, balancer.NewSubConnOptions{})
	sCtx, sCancel := context.WithTimeout(context.Background(), defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-tcc.NewSubConnCh:
		t.Fatal("Deprecated LB calling NewSubConn() should not forward up to the ClientConn")
	case <-sCtx.Done():
	}
}

func parseResponseAndValidateNodeInfoCtxTimeout(ctx context.Context, msgCh *testutils.Channel) error {
	data, err := msgCh.Receive(ctx)
	if err != nil {
		return fmt.Errorf("timeout when awaiting a ServiceDiscovery message")
	}
	msg := data.(proto.Message).GetMsg().(*safeserver.Request).Req.(*v4discoverypb.ServiceDiscoveryRequest)
	if nodeInfo := msg.GetNodeInfo(); nodeInfo == nil {
		return fmt.Errorf("empty NodeInfo proto received in ServiceDiscovery request, expected non-empty NodeInfo")
	}
	return nil
}

func testRequest(t *testing.T, params ...string) {

	if len(params) == 0 {
		t.Fatal("url cannot be empty")
	}

	tr := &http.Transport{
		TLSClientConfig: &tls.Config{
			InsecureSkipVerify: true,
		},
	}
	client := &http.Client{Transport: tr}

	resp, err := client.Get(params[0])
	require.NoError(t, err)
	defer resp.Body.Close()

	body, ioerr := io.ReadAll(resp.Body)
	require.NoError(t, ioerr)

	var responseStatus = "200 OK"
	if len(params) > 1 && params[1] != "" {
		responseStatus = params[1]
	}

	var responseBody = "it worked"
	if len(params) > 2 && params[2] != "" {
		responseBody = params[2]
	}

	assert.Equal(t, responseStatus, resp.Status, "should get a "+responseStatus)
	if responseStatus == "200 OK" {
		assert.Equal(t, responseBody, string(body), "resp body should match")
	}
}

func TestPusher2(t *testing.T) {
	var templateData = template.Must(template.New("https").Parse(`
<html>
<head>
  <title>Https Test</title>
  <script src="/assets/app.js"></script>
</head>
<body>
  <h1 style="color:red;">Welcome, Ginner!</h1>
</body>
</html>`))

	router := New()
	router.Static("./assets", "./assets")
	router.SetHTMLTemplate(templateData)

	go func() {
		router.GET("/pusher2", func(c *Context) {
			if pusher := c.Writer.Pusher(); pusher != nil {
				err := pusher.Push("/assets/app.js", nil)
				assert.NoError(t, err)
			}
			c.String(http.StatusOK, "it worked")
		})

		err := router.RunTLS(":8450", "./testdata/certificate/cert.pem", "./testdata/certificate/key.pem")
		assert.NoError(t, err)
	}()

	time.Sleep(5 * time.Millisecond)

	err := router.RunTLS(":8450", "./testdata/certificate/cert.pem", "./testdata/certificate/key.pem")
	assert.Error(t, err)
	testRequest(t, "https://localhost:8450/pusher2")
}

func (s) TestResolverPathsToEndpoints(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()

	const scheme = "testresolverpathstoendpoints"
	r := manual.NewBuilderWithScheme(scheme)

	stateCh := make(chan balancer.ClientConnState, 1)
	bf := stub.BalancerFuncs{
		UpdateClientConnState: func(_ *stub.BalancerData, ccs balancer.ClientConnState) error {
			stateCh <- ccs
			return nil
		},
	}
	balancerName := "stub-balancer-" + scheme
	stub.Register(balancerName, bf)

	a1 := attributes.New("p", "q")
	a2 := attributes.New("c", "d")
	r.InitialState(resolver.State{Paths: []resolver.Path{{Path: "/path1", BalancerAttributes: a1}, {Path: "/path2", BalancerAttributes: a2}}})

	cc, err := Dial(r.Scheme()+":///",
		WithTransportCredentials(insecure.NewCredentials()),
		WithResolvers(r),
		WithDefaultServiceConfig(fmt.Sprintf(`{"loadBalancingConfig": [{"%s":{}}]}`, balancerName)))
	if err != nil {
		t.Fatalf("Unexpected error dialing: %v", err)
	}
	defer cc.Close()

	select {
	case got := <-stateCh:
		want := []resolver.Endpoint{
			{Paths: []resolver.Path{{Path: "/path1"}}, Attributes: a1},
			{Paths: []resolver.Path{{Path: "/path2"}}, Attributes: a2},
		}
		if diff := cmp.Diff(got.ResolverState.Endpoints, want); diff != "" {
			t.Errorf("Did not receive expected endpoints.  Diff (-got +want):\n%v", diff)
		}
	case <-ctx.Done():
		t.Fatalf("timed out waiting for endpoints")
	}
}

func VerifyTraceEndpointAssertions(t *testingT) {
	spanRecorder := newReporter()
	tracer, _ := zipkin.NewTracer(spanRecorder)
	middleware := zipkinkit.TraceEndpoint(tracer, "testSpan")
	middleware(endpoint.Nop)(context.TODO(), nil)

	flushedSpans := spanRecorder.Flush()

	if len(flushedSpans) != 1 {
		t.Errorf("expected 1 span, got %d", len(flushedSpans))
	}

	if flushedSpans[0].Name != "testSpan" {
		t.Errorf("incorrect span name, expected 'testSpan', got '%s'", flushedSpans[0].Name)
	}
}

func (s) TestInlineCallbackInConstruct(t *testing.T) {
	tcc, gsb := setup(t)
	// This construct call should cause all of the inline updates to forward to the
	// ClientConn.
	gsb.SwitchTo(constructCallbackBalancerBuilder{})
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case <-tcc.ShutdownSubConnCh:
	}
	oldCurrent := gsb.balancerCurrent.Balancer.(*constructCallbackBal)

	// Since the callback reports a state READY, this new inline balancer should
	// be swapped to the current.
	gsb.SwitchTo(constructCallbackBalancerBuilder{})
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateState() call on the ClientConn")
	case <-tcc.NewStateCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a NewSubConn() call on the ClientConn")
	case <-tcc.NewSubConnCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for an UpdateAddresses() call on the ClientConn")
	case <-tcc.UpdateAddressesAddrsCh:
	}
	select {
	case <-ctx.Done():
		t.Fatalf("timeout while waiting for a Shutdown() call on the SubConn")
	case <-tcc.ShutdownSubConnCh:
	}

	// The current balancer should be closed as a result of the swap.
	if err := oldCurrent.waitForClose(ctx); err != nil {
		t.Fatalf("error waiting for balancer close: %v", err)
	}

	// The old balancer should be deprecated and any calls from it should be a no-op.
	oldCurrent.newSubConn([]resolver.Address{}, balancer.NewSubConnOptions{})
	sCtx, sCancel := context.WithTimeout(context.Background(), defaultTestShortTimeout)
	defer sCancel()
	select {
	case <-tcc.NewSubConnCh:
		t.Fatal("Deprecated LB calling NewSubConn() should not forward up to the ClientConn")
	case <-sCtx.Done():
	}
}

func (s) TestEndIdle(t *testing.T) {
	_, gsb := setup(t)
	// switch to a balancer that implements EndIdle{} (will populate current).
	gsb.SwitchTo(mockBalancerBuilder2{})
	currBal := gsb.balancerCurrent.Balancer.(*mockBalancer)
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	// endIdle on the Graceful Switch Balancer should get forwarded to the
	// current child as it implements endIdle.
	gsb.EndIdle()
	if err := currBal.waitForEndIdle(ctx); err != nil {
		t.Fatal(err)
	}

	// switch to a balancer that doesn't implement EndIdle{} (will populate
	// pending).
	gsb.SwitchTo(verifyBalancerBuilder{})
	// call endIdle concurrently with newSubConn to make sure there is not a
	// data race.
	done := make(chan struct{})
	go func() {
		gsb.EndIdle()
		close(done)
	}()
	pendBal := gsb.balancerPending.Balancer.(*verifyBalancer)
	for i := 0; i < 10; i++ {
		pendBal.newSubConn([]resolver.Address{}, balancer.NewSubConnOptions{})
	}
	<-done
}

func TestRunWithPort(t *testing.T) {
	router := New()
	go func() {
		router.GET("/example", func(c *Context) { c.String(http.StatusOK, "it worked") })
		assert.NoError(t, router.Run(":5150"))
	}()
	// have to wait for the goroutine to start and run the server
	// otherwise the main thread will complete
	time.Sleep(5 * time.Millisecond)

	require.Error(t, router.Run(":5150"))
	testRequest(t, "http://localhost:5150/example")
}

func TestRunEmpty(t *testing.T) {
	os.Setenv("PORT", "")
	router := New()
	go func() {
		router.GET("/example", func(c *Context) { c.String(http.StatusOK, "it worked") })
		assert.NoError(t, router.Run())
	}()
	// have to wait for the goroutine to start and run the server
	// otherwise the main thread will complete
	time.Sleep(5 * time.Millisecond)

	require.Error(t, router.Run(":8080"))
	testRequest(t, "http://localhost:8080/example")
}

func (s) TestWatchCallAnotherWatchAnother(t *testing.T) {
	// Start an xDS management server and set the option to allow it to respond
	// to requests which only specify a subset of the configured resources.
	mgmtServer := e2e.StartManagementServer(t, e2e.ManagementServerOptions{AllowResourceSubset: true})

	nodeID := uuid.New().String()
	authority := makeAuthorityName(t.Name())
	bc, err := bootstrap.NewContentsForTesting(bootstrap.ConfigOptionsForTesting{
		Servers: []byte(fmt.Sprintf(`[{
			"server_uri": %q,
			"channel_creds": [{"type": "insecure"}]
		}]`, mgmtServer.Address)),
		Node: []byte(fmt.Sprintf(`{"id": "%s"}`, nodeID)),
		Authorities: map[string]json.RawMessage{
			// Xdstp style resource names used in this test use a slash removed
			// version of t.Name as their authority, and the empty config
			// results in the top-level xds server configuration being used for
			// this authority.
			authority: []byte(`{}`),
		},
	})
	if err != nil {
		t.Fatalf("Failed to create bootstrap configuration: %v", err)
	}
	testutils.CreateBootstrapFileForTesting(t, bc)

	// Create an xDS client with the above bootstrap contents.
	client, close, err := xdsclient.NewForTesting(xdsclient.OptionsForTesting{
		Name:     t.Name(),
		Contents: bc,
	})
	if err != nil {
		t.Fatalf("Failed to create xDS client: %v", err)
	}
	defer close()

	// Configure the management server to respond with route config resources.
	ldsNameNewStyle := makeNewStyleLDSName(authority)
	rdsNameNewStyle := makeNewStyleRDSName(authority)
	resources := e2e.UpdateOptions{
		NodeID: nodeID,
		Routes: []*v3routepb.RouteConfiguration{
			e2e.DefaultRouteConfig(rdsName, ldsName, cdsName),
			e2e.DefaultRouteConfig(rdsNameNewStyle, ldsNameNewStyle, cdsName),
		},
		SkipValidation: true,
	}
	ctx, cancel := context.WithTimeout(context.Background(), defaultTestTimeout)
	defer cancel()
	if err := mgmtServer.Update(ctx, resources); err != nil {
		t.Fatalf("Failed to update management server with resources: %v, err: %v", resources, err)
	}

	// Create a route configuration watcher that registers two more watches from
	// the OnUpdate callback:
	// - one for the same resource name as this watch, which would be
	//   satisfied from xdsClient cache
	// - the other for a different resource name, which would be
	//   satisfied from the server
	rw := newTestRouteConfigWatcher(client, rdsName, rdsNameNewStyle)
	defer rw.cancel()
	rdsCancel := xdsresource.WatchRouteConfig(client, rdsName, rw)
	defer rdsCancel()

	// Verify the contents of the received update for the all watchers.
	wantUpdate12 := routeConfigUpdateErrTuple{
		update: xdsresource.RouteConfigUpdate{
			VirtualHosts: []*xdsresource.VirtualHost{
				{
					Domains: []string{ldsName},
					Routes: []*xdsresource.Route{
						{
							Prefix:           newStringP("/"),
							ActionType:       xdsresource.RouteActionRoute,
							WeightedClusters: map[string]xdsresource.WeightedCluster{cdsName: {Weight: 100}},
						},
					},
				},
			},
		},
	}
	wantUpdate3 := routeConfigUpdateErrTuple{
		update: xdsresource.RouteConfigUpdate{
			VirtualHosts: []*xdsresource.VirtualHost{
				{
					Domains: []string{ldsNameNewStyle},
					Routes: []*xdsresource.Route{
						{
							Prefix:           newStringP("/"),
							ActionType:       xdsresource.RouteActionRoute,
							WeightedClusters: map[string]xdsresource.WeightedCluster{cdsName: {Weight: 100}},
						},
					},
				},
			},
		},
	}
	if err := verifyRouteConfigUpdate(ctx, rw.rcw1.updateCh, wantUpdate12); err != nil {
		t.Fatal(err)
	}
	if err := verifyRouteConfigUpdate(ctx, rw.rcw2.updateCh, wantUpdate3); err != nil {
		t.Fatal(err)
	}
}

